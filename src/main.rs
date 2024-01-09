use std::{net::Ipv4Addr, time::Duration, error::Error, path::Path, fs::File, io, str::FromStr, sync::OnceLock};

use dnx_rs::{Tree, TreeSortable};
use hickory_server::{server::{RequestHandler, ResponseHandler, Request, ResponseInfo}, proto::{op::{Header, ResponseCode, OpCode}, rr::LowerName}, ServerFuture, authority::MessageResponseBuilder};
use tokio::net::{UdpSocket, TcpListener};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const TCP_TIMEOUT: Duration = Duration::from_secs(10);

static DOMAIN_TREE: OnceLock<Tree<String,DnxEntry>> = OnceLock::new();

static DEFAULT_SERVER: OnceLock<DnxEntry> = OnceLock::new();

struct DnxRequestHandler;

impl DnxRequestHandler {
    async fn do_handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> Result<ResponseInfo, Box<dyn Error>> {
        let builder = MessageResponseBuilder::from_message_request(request);
        let mut header = Header::response_from_request(request.header());

        match request.op_code() {
            OpCode::Query => {
                let tree = DOMAIN_TREE.get().unwrap();
                let entry = tree.find(request.query().name()).unwrap_or_else(|| DEFAULT_SERVER.get().unwrap());
            }
            _ => ()
        }

        todo!()
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

#[derive(Serialize, Deserialize, Clone)]
struct DnxNatEntry {
    ip_original: Ipv4Addr,
    ip_translation: Ipv4Addr,
    mask: Ipv4Addr,
}

#[derive(Serialize, Deserialize, Clone)]
struct DnxEntry {
    zone: String,
    server: Ipv4Addr,
    nat: Option<DnxNatEntry>,
}

#[derive(Serialize, Deserialize)]
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
    fn matches(&self, name: &LowerName) -> bool {
        let zone = LowerName::from_str(&self.zone).unwrap();
        zone.zone_of(name)
    }

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

    let mut tree = Tree::new();
    config.zones.iter().for_each(|entry| {
        tree.insert(entry.clone());
    });

    _ = DOMAIN_TREE.set(tree);
    _ = DEFAULT_SERVER.set(DnxEntry{
        zone: "".to_string(),
        server: config.default_server,
        nat: None,
    });

    config
}

#[tokio::main]
async fn main() {
    let config = load_config();

    let mut server = ServerFuture::new(DnxRequestHandler);

    let udp_socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, config.udp_port)).await.unwrap();
    let tcp_socket = TcpListener::bind((Ipv4Addr::UNSPECIFIED, config.tcp_port)).await.unwrap();

    server.register_socket(udp_socket);
    server.register_listener(tcp_socket, TCP_TIMEOUT);

    let _ = server.block_until_done().await;
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
    fn test_dnx_entry_matches() {
        let dnx_entry = DnxEntry {
            zone: "example.com".to_string(),
            server: Ipv4Addr::new(192, 168, 0, 1),
            nat: None,
        };

        assert!(dnx_entry.matches(&LowerName::from_str("my.subdomain.example.com").unwrap()));
        assert!(!dnx_entry.matches(&LowerName::from_str("my.subdomain.example.org").unwrap()));
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

