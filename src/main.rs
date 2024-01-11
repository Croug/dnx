use std::{
    net::Ipv4Addr,
    time::Duration,
    error::Error,
    path::Path,
    fs::File,
    io,
    collections::HashMap,
};

use dnx_rs::{
    Tree,
    TreeSortable,
};

use hickory_server::{
    server::{
        RequestHandler,
        ResponseHandler,
        Request,
        ResponseInfo,
    },
    proto::op::{
        Header,
        ResponseCode,
        OpCode,
    },
    ServerFuture,
    authority::MessageResponseBuilder,
};

use hickory_resolver::{
    TokioAsyncResolver,
    config::{
        ResolverConfig,
        ResolverOpts,
        NameServerConfig,
        Protocol,
    }, proto::rr::{Record, RecordType, RData},
};

use tokio::{net::{
    UdpSocket,
    TcpListener,
}, sync::RwLock};

use serde::{
    de::DeserializeOwned,
    Deserialize,
    Serialize,
};

const TCP_TIMEOUT: Duration = Duration::from_secs(10);

struct DnxRequestHandler {
    tree: Tree<String, DnxEntry>,
    default_server: DnxEntry,
    resolvers: RwLock<HashMap<String, TokioAsyncResolver>>,
}

impl DnxRequestHandler {
    fn from_config(config: DnxConfig) -> Self{
        let mut tree = Tree::new();
        config.zones.iter().for_each(|entry| {
            tree.insert(entry.clone());
        });

        Self {
            tree,
            default_server: DnxEntry {
                zone: "".to_string(),
                server: config.default_server,
                nat: None,
            },
            resolvers: RwLock::new(HashMap::new()),
        }
    }
    async fn do_handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> Result<ResponseInfo, Box<dyn Error>> {
        let builder = MessageResponseBuilder::from_message_request(request);
        let mut header = Header::response_from_request(request.header());

        Ok(match request.op_code() {
            OpCode::Query => {
                let query = request.query();
                let name = query.name();
                let entry = self.tree.find(name).unwrap_or(&self.default_server);
                log::trace!("Found entry: {:?}", entry);
                let resolver = self.get_resolver(entry).await;
                log::trace!("Starting lookup for: {}", name);
                let upstream_response = resolver.lookup(name, query.query_type()).await?;
                log::trace!("Got upstream response: {:?}", upstream_response);
                let records: Vec<Record> = upstream_response.record_iter().map(|record| {
                    match record.record_type() {
                        RecordType::A => {
                            let parts = record.clone().into_parts();
                            let rdata = parts.rdata.unwrap();
                        
                            let ip = if let RData::A(ip) = rdata {
                                ip
                            } else {
                                panic!("Non A Record RData in A Record");
                            };
                        
                            let ip = entry.translate(ip.into());
                        
                            let mut record = Record::new();
                            record.set_record_type(parts.rr_type);
                            record.set_data(Some(RData::A(ip.into())));
                            record.set_dns_class(parts.dns_class);
                            record.set_name(parts.name_labels);
                            record.set_ttl(parts.ttl);

                            record
                        }
                        _ => record.clone(),
                    }
                }).collect();
                let response = builder.build(header, records.iter(), &[], &[], &[]);
                response_handle.send_response(response).await?
            }
            _ => {
                header.set_response_code(ResponseCode::NotImp);
                let response = builder.build(header, &[], &[], &[], &[]);
                response_handle.send_response(response).await?
            }
        })
    }
    async fn get_resolver(&self, entry: &DnxEntry) -> TokioAsyncResolver {
        let resolver = {
            self.resolvers.read().await.get(&entry.zone).cloned()
        };
    
        match resolver {
            Some(resolver) => {
                log::trace!("Using cached resolver for zone: {}", entry.zone);
                resolver
            }
            None => {
                log::debug!("Creating resolver for zone: {}", entry.zone);
                let options = ResolverOpts::default();

                let nameservers = vec![NameServerConfig::new((entry.server, 53).into(), Protocol::Udp)];
                let config = ResolverConfig::from_parts(None, vec![], nameservers);

                let resolver = TokioAsyncResolver::tokio(config, options);
                self.resolvers.write().await.entry(entry.zone.clone()).or_insert(resolver).clone()
            }
        }
    }
}

#[async_trait::async_trait]
impl RequestHandler for DnxRequestHandler {
    async fn handle_request<R: ResponseHandler>(&self, request: &Request, response_handle: R) -> ResponseInfo {
        match self.do_handle_request(request, response_handle).await {
            Ok(info) => info,
            Err(e) => {
                eprintln!("Error handling request: {}", e);
                let mut header = Header::new();
                header.set_response_code(ResponseCode::ServFail);
                header.into()
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct DnxNatEntry {
    ip_original: Ipv4Addr,
    ip_translation: Ipv4Addr,
    mask: Ipv4Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct DnxEntry {
    zone: String,
    server: Ipv4Addr,
    nat: Option<DnxNatEntry>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct DnxConfig {
    pub zones: Vec<DnxEntry>,
    pub tcp_port: u16,
    pub udp_port: u16,
    pub default_server: Ipv4Addr,
}

impl TreeSortable<String> for DnxEntry {
    fn get_path(&self) -> Vec<String> {
        self.zone.get_path()
    }
}

impl DnxNatEntry {
    fn matches(&self, ip: Ipv4Addr) -> bool {
        let mask = u32::from(self.mask);
        let ip = u32::from(ip);
        let nat = u32::from(self.ip_original);
    
        (ip & mask) == (nat & mask)
    }

    fn translate(&self, ip: Ipv4Addr) -> Ipv4Addr {
        if !self.matches(ip) {
            return ip;
        }

        let mask = u32::from(self.mask);
        let ip = u32::from(ip);
        let nat = u32::from(self.ip_translation);

        let ip = ip & !mask;
        let nat = nat & mask;

        Ipv4Addr::from(ip | nat)
    }
}

impl DnxEntry {
    fn translate(&self, ip: Ipv4Addr) -> Ipv4Addr {
        match self.nat {
            None => ip,
            Some(ref nat) => {
                nat.translate(ip)
            }
        }
    }
}

impl Default for DnxConfig {
    fn default() -> Self {
        DnxConfig {
            zones: Vec::new(),
            tcp_port: 53,
            udp_port: 53,
            default_server: Ipv4Addr::new(1, 1, 1, 1),
        }
    }
}

fn save_json<T: Serialize, P: AsRef<Path>>(data: &T, path: P) -> io::Result<()> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, data)?;
    Ok(())
}

fn load_json<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> io::Result<T> {
    let file = File::open(path)?;
    let data = serde_json::from_reader(file)?;
    Ok(data)
}

fn load_config() -> DnxConfig {
    let config = load_json("dnx.json").unwrap_or_else(|_| {
        let mut config = DnxConfig::default();
        config.zones.push(DnxEntry{
            zone: "example.com".to_string(),
            server: Ipv4Addr::new(192, 168, 0, 1),
            nat: Some(DnxNatEntry {
                ip_original: Ipv4Addr::new(192, 168, 0, 0),
                ip_translation: Ipv4Addr::new(10, 0, 0, 0),
                mask: Ipv4Addr::new(255, 255, 0, 0),
            }),
        });
        save_json(&config, "dnx.json").unwrap();
        config
    });

    config
}

async fn setup_server() -> io::Result<ServerFuture<DnxRequestHandler>> {
    let config = load_config();

    let mut server = ServerFuture::new(DnxRequestHandler::from_config(config.clone()));

    let udp_socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, config.udp_port)).await.unwrap();
    let tcp_socket = TcpListener::bind((Ipv4Addr::UNSPECIFIED, config.tcp_port)).await.unwrap();

    server.register_socket(udp_socket);
    server.register_listener(tcp_socket, TCP_TIMEOUT);

    Ok(server)
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let mut server = setup_server().await.unwrap();

    tokio::signal::ctrl_c().await.unwrap();

    server.shutdown_gracefully().await.expect("Failed to shutdown gracefully");

    println!("Goodbye!");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dnx_nat_entry_matches() {
        let nat_entry = DnxNatEntry {
            ip_original: Ipv4Addr::new(192, 168, 0, 0),
            ip_translation: Ipv4Addr::new(10, 0, 0, 0),
            mask: Ipv4Addr::new(255, 255, 0, 0),
        };

        assert!(nat_entry.matches(Ipv4Addr::new(192, 168, 1, 1)));
        assert!(!nat_entry.matches(Ipv4Addr::new(10, 0, 0, 1)));
    }

    #[test]
    fn test_dnx_nat_entry_translate() {
        let nat_entry = DnxNatEntry {
            ip_original: Ipv4Addr::new(192, 168, 0, 0),
            ip_translation: Ipv4Addr::new(10, 0, 0, 0),
            mask: Ipv4Addr::new(255, 255, 0, 0),
        };

        assert_eq!(
            nat_entry.translate(Ipv4Addr::new(192, 168, 1, 1)),
            Ipv4Addr::new(10, 0, 1, 1)
        );
        assert_eq!(
            nat_entry.translate(Ipv4Addr::new(10, 0, 0, 1)),
            Ipv4Addr::new(10, 0, 0, 1)
        );
    }

    #[test]
    fn test_dnx_entry_translate() {
        let dnx_entry = DnxEntry {
            zone: "example.com".to_string(),
            server: Ipv4Addr::new(192, 168, 0, 1),
            nat: Some(DnxNatEntry {
                ip_original: Ipv4Addr::new(192, 168, 0, 0),
                ip_translation: Ipv4Addr::new(10, 0, 0, 0),
                mask: Ipv4Addr::new(255, 255, 0, 0),
            }),
        };

        assert_eq!(
            dnx_entry.translate(Ipv4Addr::new(192, 168, 1, 1)),
            Ipv4Addr::new(10, 0, 1, 1)
        );
        assert_eq!(
            dnx_entry.translate(Ipv4Addr::new(10, 0, 0, 1)),
            Ipv4Addr::new(10, 0, 0, 1)
        );
    }
}

