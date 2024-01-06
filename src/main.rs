use std::{net::Ipv4Addr, time::Duration, error::Error, path::Path, fs::File, io, str::FromStr};

use hickory_server::{server::{RequestHandler, ResponseHandler, Request, ResponseInfo}, proto::{op::{Header, ResponseCode}, rr::{LowerName, Name}}, ServerFuture, authority::MessageResponseBuilder};
use tokio::net::{UdpSocket, TcpListener};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

const TCP_TIMEOUT: Duration = Duration::from_secs(10);

struct DnxRequestHandler;

impl DnxRequestHandler {
    async fn do_handle_request<R: ResponseHandler>(
        &self,
        request: &Request,
        mut response_handle: R,
    ) -> Result<ResponseInfo, Box<dyn Error>> {
        let builder = MessageResponseBuilder::from_message_request(request);
        let mut header = Header::response_from_request(request.header());

        //

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

#[derive(Serialize, Deserialize)]
struct DnxNatEntry {
    ip: Ipv4Addr,
    mask: Ipv4Addr,
}

#[derive(Serialize, Deserialize)]
struct DnxEntry {
    zone: String,
    server: Ipv4Addr,
    nat: Option<DnxNatEntry>,
}

impl DnxNatEntry {
    fn matches(&self, ip: Ipv4Addr) -> bool {
        let mask = u32::from(self.mask);
        let ip = u32::from(ip);
        let nat = u32::from(self.ip);
    
        (ip & mask) == (nat & mask)
    }

    fn translate(&self, ip: Ipv4Addr) -> Ipv4Addr {
        if !self.matches(ip) {
            return ip;
        }

        let mask = u32::from(self.mask);
        let ip = u32::from(ip);
        let nat = u32::from(self.ip);

        let ip = ip & !mask;
        let nat = nat & mask;

        Ipv4Addr::from(ip | nat)
    }
}

impl DnxEntry {
    fn matches(&self, name: &LowerName) -> bool {
        let zone = LowerName::from(Name::from_str(self.zone.as_str()).unwrap());
        name.zone_of(&zone)
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

fn save_json<T: Serialize, P: AsRef<Path>>(data: &T, path: P) -> io::Result<()> {
    let mut file = File::create(path)?;
    serde_json::to_writer(file, data)?;
    Ok(())
}

fn load_json<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> io::Result<T> {
    let file = File::open(path)?;
    let data = serde_json::from_reader(file)?;
    Ok(data)
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let mut server = ServerFuture::new(DnxRequestHandler);

    server.register_socket(UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 53)).await.unwrap());
    server.register_listener(TcpListener::bind((Ipv4Addr::UNSPECIFIED, 53)).await.unwrap(), TCP_TIMEOUT);

    let _ = server.block_until_done().await;
}
