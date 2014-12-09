extern crate url;

use std::collections;
use std::comm;
use std::default::Default;
use std::io;
use std::time;
use std::string;
use std::io::timer;
use std::io::net::ip;
use std::io::net::addrinfo;

use url::{Url};

static VERSION: &'static str = "0.0.1";

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

fn make_connection(host: &ip::IpAddr, port: ip::Port, timeout: time::Duration) -> io::IoResult<io::TcpStream> {
    let s = ip::SocketAddr{ip: *host, port: port};
    return io::TcpStream::connect_timeout(s, timeout);
}

// XXX, there must be a better way of writing this.
fn find_working_addr(addrs: Vec<ip::IpAddr>, port: ip::Port, timeout: time::Duration) -> Option<io::TcpStream> {
    for addr in addrs.iter() {
        match make_connection(addr, port, timeout) {
            Err(e) => {
                println!("{}", e);
                false
            }
            Ok(s) => { 
                return Some(s);
            }
        };
    }
    return None
}

// XXX: see https://github.com/rust-lang/rust/issues/19650
// pub type Header = collections::HashMap<String, Vec<String>>;

pub struct RequestOptions {
    headers: collections::HashMap<String, Vec<String>>,
    verify: bool,
    // XXX: should these be str's ?
    data: Option<collections::HashMap<String, Vec<String>>>,
    params: Option<collections::HashMap<String, Vec<String>>>,
    auth: Option<Vec<String>>,
    timeout: time::Duration,
    connect_timeout: time::Duration,
    dns_timeout: time::Duration,
}

impl Default for RequestOptions {
    fn default() -> RequestOptions {
        let mut h = collections::HashMap::new();
        let vsn = format!("rustclient/{}", VERSION);
        h.insert("User-Agent".to_string(), vec![vsn]);
        let nodata: Option<collections::HashMap<String, Vec<String>>> = None;
        let noparams: Option<collections::HashMap<String, Vec<String>>> = None;
        let noauth: Option<Vec<String>> = None;
        return RequestOptions{ 
            headers: h,
            verify: true,
            data: nodata,
            params: noparams,
            auth: noauth,
            timeout: time::Duration::seconds(30),
            dns_timeout: time::Duration::seconds(30),
            connect_timeout: time::Duration::seconds(30),
        }
    }
}

/// Make a HTTP request and return a response (eventually)
pub fn get(raw_url: &str, ro: RequestOptions) -> bool {
    let parsed_url = Url::parse(raw_url);
    let url = match parsed_url {
        Ok(url) => { url }
        Err(e) => {
            println!("{}", e);
            return false;
        }
    };
    let path = match url.serialize_path() {
        Some(p) => { p }
        None => { return false }
    };
    let port = get_port(&url);
    let dom = match url.domain() {
        Some(d) => { d }
        None => { return false ; }
    };
    if domain_is_ipaddr(dom) {
    } else {
        let maybeaddrs = lookup(dom, ro.dns_timeout);
        let addrs = match maybeaddrs {
            Ok(addrs) => { addrs }
            Err(e) => {
                println!("{}", e);
                return false;
            }
        };
        let mut sock = match find_working_addr(addrs, port, ro.connect_timeout) {
            Some(s) => { s }
            None => {
                println!("coludnt establish connection");
                return false;
            }
        };
        let mut request_buf = String::new();
        let topline = format!("GET {} HTTP/1.0\r\n", path);
        request_buf.push_str(topline.as_slice());
        for (key, val_tuple) in ro.headers.iter() {
            for val in val_tuple.iter() {
                let hdr = format!("{key}: {value}\r\n", key=key, value=val);
                request_buf.push_str(hdr.as_slice());
            }
        }
        request_buf.push_str("\r\n");
        println!("{}", request_buf);
        sock.write(request_buf.as_bytes());
        let mut reader = io::BufferedReader::new(sock);
        let rtopline = match reader.read_to_end() {
            Ok(rt) => { rt }
            Err(e) => {
                println!("{}", e);
                return false;
            }
        };
        println!("{}", string::String::from_utf8(rtopline));
    }
    return true;
}

#[test]
fn test_get() {
    let ropts = RequestOptions{
        timeout: time::Duration::seconds(1),
        dns_timeout: time::Duration::seconds(1),
        connect_timeout: time::Duration::seconds(1),
        ..Default::default()
    };
    get("http://api.twilio.com", ropts);
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
