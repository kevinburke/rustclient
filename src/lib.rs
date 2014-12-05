extern crate url;

use std::comm;
use std::io;
use std::time;
use std::io::timer;
use std::io::net::ip;
use std::io::net::addrinfo;

use url::{Url, ParseError};

/// Find a list of IP addresses for the given host. Give up after
/// `timeout_duration` if the DNS server does not return a response.
pub fn lookup(host: &str, timeout_duration: time::Duration) -> io::IoResult<Vec<ip::IpAddr>> {
    let ownedhost = host.into_string();
    let (tx, rx): (Sender<io::IoResult<Vec<ip::IpAddr>>>, Receiver<io::IoResult<Vec<ip::IpAddr>>>) = comm::channel();
    let mut t = timer::Timer::new().unwrap();
    let timeout = t.oneshot(timeout_duration);

    let detail = format!("Failed to resolve {} after {} milliseconds", 
                         ownedhost, timeout_duration.num_milliseconds());

    spawn(proc() {
        tx.send(addrinfo::get_host_addresses(ownedhost.as_slice()));
    });

    loop {
        select! {
            val = rx.recv() => return val,
            () = timeout.recv() => {
                let e = io::IoError{
                    kind: io::IoErrorKind::TimedOut,
                    desc: "DNS lookup timed out",
                    detail: Some(detail)
                };
                return Err(e)
            }
        }
    }
}

/// Checks if a domain is an IP address or a hostname
fn domain_is_ipaddr(domain: &str) -> bool {
    return from_str::<ip::IpAddr>(domain).is_some()
}

fn get_port(url: &Url) -> u16 {
    let maybeport = url.port();
    match maybeport {
        Some(port) => {
            port
        }
        None => {
            if url.scheme == "https" {
                443
            } else {
                80
            }
        }
    }
}

fn make_connection(host: &ip::IpAddr, port: ip::Port) -> io::IoResult<TcpStream> {
    let s = ip::SocketAddr{ip: *host, port: port};
    return io::TcpStream::connect_timeout(s, time::Duration::milliseconds(3100)).unwrap();
}

/// Make a HTTP request and return a response (eventually)
pub fn get(raw_url: &str) -> bool {
    let parsed_url = Url::parse(raw_url);
    match parsed_url {
        Ok(url) => {
            let port = get_port(&url);
            match url.domain() {
                Some(d) => {
                    if domain_is_ipaddr(d) {
                    } else {
                        let maybeaddrs = lookup(d, time::Duration::seconds(30));
                        match maybeaddrs {
                            Ok(addrs) => {
                                for addr in addrs.iter() {
                                    match make_connection(addr, port) {
                                        Ok(sock) => {

                                        }
                                        Err(e) => {
                                            println!("{}", e);
                                            return false;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                println!("{}", e);
                                return false;
                            }
                        }
                    }
                }
                None => { return false ; }
            }
        }
        Err(e) => {
            println!("{}", e);
            return false;
        }
    }
    return true;
}

#[test]
fn test_get() {
    get("https://api.twilio.com");
    assert!(false)
}

#[test]
fn test_domain() {
    assert!(domain_is_ipaddr("10.0.0.1"));
    assert!(domain_is_ipaddr("::1"));
    assert!(!domain_is_ipaddr("api.twilio.com"));
    assert!(!domain_is_ipaddr("foo"));
}

#[test]
fn test_get_port() {
    let httpsuri = Url::parse("https://api.twilio.com").unwrap();
    assert_eq!(get_port(&httpsuri), 443)

    let httpsuriport = Url::parse("https://api.twilio.com:5678").unwrap();
    assert_eq!(get_port(&httpsuriport), 5678)

    let httpuriport = Url::parse("http://api.twilio.com:5678").unwrap();
    assert_eq!(get_port(&httpuriport), 5678)

    let httpuri = Url::parse("http://api.twilio.com").unwrap();
    assert_eq!(get_port(&httpuri), 80)
}
