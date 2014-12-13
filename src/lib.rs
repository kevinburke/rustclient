extern crate serialize;
extern crate url;

use serialize::json;
use std::collections;
use std::comm;
use std::default::Default;
use std::io;
use std::time;
use std::io::timer;
use std::io::net::ip;
use std::io::net::addrinfo;

use url::{Url};
use url::form_urlencoded;

static VERSION: &'static str = "0.0.1";

pub enum Body {
    FormUrlEncoded(collections::HashMap<String, Vec<String>>),
    JSON(json::Json),
}

// ugh. rust-url requires [(key, val)] combos, but we have {key: [val1, val2]}
fn map_to_vec(map: collections::HashMap<String, Vec<String>>) -> Vec<(String, String)> {
    let mut vec = Vec::new();
    for (key, val_tuple) in map.iter() {
        for val in val_tuple.iter() {
            let ownedkey = key.clone();
            let ownedval = val.clone();
            vec.push((ownedkey, ownedval));
        }
    }
    vec
}

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

fn parse_version(httpvsn: &str) -> Result<int, &str> {
    match httpvsn {
        "HTTP/0.9" => Ok(9),
        "HTTP/1.0" => Ok(10),
        "HTTP/1.1" => Ok(11),
        _ => {
            let msg = format!("Bad status line: {}", httpvsn);
            // XXX: why clone here? 
            Err(msg.as_slice().clone())
        }
    }
}

fn parse_topline(topline: &str) -> Result<(int, int, &str), &str> {
    // XXX read the RFC for http responses, is whitespace ok, etc.
    let splits: Vec<&str> = topline.splitn(2, ' ').collect();
    let (httpvsn, status_str, rest) = match splits.len() {
        0 | 1 => return Err("Too few values"),
        2 => {
            let httpvsn = splits[0];
            let status = splits[1];
            (httpvsn, status, "")
        },
        3 => {
            let httpvsn = splits[0];
            let status_str = splits[1];
            let rest = splits[2];
            (httpvsn, status_str, rest)
        },
        _ => { return Err("Too many values") }
    };
    let vsn = match parse_version(httpvsn) {
        Ok(vsn) => { vsn }
        Err(e) => { return Err(e) }
    };
    let status = match from_str(status_str) {
        Some(status) => status,
        None => {
            let msg = format!("Bad status line: {}", topline);
            return Err(msg.as_slice().clone())
        }
    };
    Ok((vsn, status, rest))
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
    data: Option<Body>,
    params: Option<collections::HashMap<String, Vec<String>>>,
    auth: Option<Vec<String>>,
    timeout: time::Duration,
    connect_timeout: time::Duration,
    dns_timeout: time::Duration,
}

pub struct Response<'r> {
    // XXX use custom types for these two.
    status: int,
    status_description: &'r str,
    version: int,
    headers: collections::HashMap<String, Vec<String>>,
    body: io::BufferedReader<io::TcpStream>,
}

impl Default for RequestOptions {
    fn default() -> RequestOptions {
        let mut h = collections::HashMap::new();
        let vsn = format!("rustclient/{}", VERSION);
        h.insert("User-Agent".to_string(), vec![vsn]);
        let nodata: Option<Body> = None;
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

pub fn get(raw_url: &str, ro: RequestOptions) -> Result<Response, &str> {
    request("GET", raw_url, ro)
}

pub fn post(raw_url: &str, data: Body, ro: RequestOptions) -> Result<Response, &str> {
    let ropost = RequestOptions{
        data: Some(data),
        ..ro
    };
    request("POST", raw_url, ropost)
}

pub fn put(raw_url: &str, data: Body, ro: RequestOptions) -> Result<Response, &str> {
    let ropost = RequestOptions{
        data: Some(data),
        ..ro
    };
    request("PUT", raw_url, ropost)
}

pub fn delete(raw_url: &str, ro: RequestOptions) -> Result<Response, &str> {
    request("DELETE", raw_url, ro)
}

fn get_body_contenttype(rodata: &Option<Body>) -> Option<(&'static str, &'static str)> {
    match *rodata {
        Some(ref body) => { match *body {
                Body::FormUrlEncoded(_) => { 
                    Some(("content-type", "application/x-www-formurlencoded"))
                }
                Body::JSON(_) => {
                    Some(("content-type", "application/json"))
                }
            }
        }
        None => { None }
    }
}
    
/// Make a HTTP request and return a response (eventually)
pub fn request<'r>(method: &str, raw_url: &str, ro: RequestOptions) -> Result<Response<'r>, &'r str> {
    let parsed_url = Url::parse(raw_url);
    let url = match parsed_url {
        Ok(url) => { url }
        Err(e) => {
            return Err("bad url")
        }
    };
    let path = match url.serialize_path() {
        Some(p) => { p }
        None => { return Err("bad path") }
    };
    let port = get_port(&url);
    let dom = match url.domain() {
        Some(d) => { d }
        None => { return Err("bad domain") ; }
    };
    let addrs = match from_str::<ip::IpAddr>(dom) {
        Some(domain) => { vec![domain] }
        None => {
            let maybeaddrs = lookup(dom, ro.dns_timeout);
            match maybeaddrs {
                Ok(addrs) => { addrs }
                Err(e) => {
                    return Err("bad address");
                }
            }
        }
    }; 
    let mut request_buf = String::new();
    let topline = format!("{method} {path} HTTP/1.0\r\n", method=method, path=path);
    request_buf.push_str(topline.as_slice());

    let mut request_headers: Vec<(&str, &str)> = vec![];
    for (key, val_tuple) in ro.headers.iter() {
        for val in val_tuple.iter() {
            request_headers.push((key.as_slice(), val.as_slice()));
        }
    }

    // 1. determine what type of body it is.
    // 2. add the correct content-type header 
    // 3. get the body as a string
    // 4. add the correct content-length header based on the string length
    match get_body_contenttype(&ro.data) {
        Some(hdrs) => {
            request_headers.push(hdrs);
        }
        None => {}
    }

    let s: &str = if ro.data.is_some() {
        let body = ro.data.unwrap();
        let s = match body {
            Body::FormUrlEncoded(map) => { 
                let vec = map_to_vec(map);
                let a = form_urlencoded::serialize_owned(vec.as_slice());
                a.as_slice().clone()
            }
            Body::JSON(j) => {
                json::encode(&j).as_slice().clone()
            }
        };
        let len = format!("{}", s.len());
        // XXX: why does this need a clone?
        let lens = len.as_slice().clone();
        request_headers.push(("content-length", lens));
        s
    } else {
        ""
    };

    for pair in request_headers.iter() {
        let &(key, value) = pair;
        let hdr = format!("{key}: {value}\r\n", key=key, value=value);
        request_buf.push_str(hdr.as_slice());
    }

    request_buf.push_str("\r\n");
    request_buf.push_str(s);
    println!("{}", request_buf);
    let mut sock = match find_working_addr(addrs, port, ro.connect_timeout) {
        Some(s) => { s }
        None => {
            return Err("coludnt establish connection");
        }
    };
    sock.write(request_buf.as_bytes());
    let mut reader = io::BufferedReader::new(sock);
    let rtopline = match reader.read_line() {
        Ok(rt) => { rt }
        Err(e) => {
            return Err("couldnt read a line");
        }
    };
    let (vsn, status, rest) = match parse_topline(rtopline.as_slice()) {
        Ok((vsn, status, rest)) => { (vsn, status, rest) }
        Err(e) => {
            return Err(e);
        }
    };
    let mut response_headers: Vec<(&str, &str)> = vec![];
    while true {
        let line = match reader.read_line() {
            Ok(rt) => { rt }
            Err(e) => {
                return Err("couldnt read a line");
            }
        };
        if is_last(&line) {
            break
        }
    }
    let r = Response{
        version: vsn,
        status_description: rest,
        status: status,
    };
    return Ok(r);
}

fn is_last(c: &String) -> bool {
    let cs = c.as_slice();
    cs == "\r\n" || cs == "\n"
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
fn test_ip_get() {
    let mut h = collections::HashMap::new();
    h.insert("Host".to_string(), vec!["jsonip.com".to_string()]);
    let ropts = RequestOptions{
        headers: h,
        timeout: time::Duration::seconds(1),
        dns_timeout: time::Duration::seconds(1),
        connect_timeout: time::Duration::seconds(1),
        ..Default::default()
    };
    get("http://96.126.98.124", ropts);
    assert!(false)
}

#[test]
fn test_post() {
    let mut b = collections::HashMap::new();
    b.insert("foo".to_string(), vec!["bar".to_string()]);
    let ropts = RequestOptions{
        timeout: time::Duration::seconds(1),
        dns_timeout: time::Duration::seconds(1),
        connect_timeout: time::Duration::seconds(1),
        ..Default::default()
    };
    post("http://api.twilio.com", Body::FormUrlEncoded(b), ropts);
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

#[test]
fn test_parse_version() {
    assert_eq!(parse_version("HTTP/0.9"), Ok(9))
    assert_eq!(parse_version("HTTP/1.0"), Ok(10))
    assert_eq!(parse_version("HTTP/1.1"), Ok(11))
    assert_eq!(parse_version(" HTTP/1.0"), Err("Bad status line:  HTTP/1.0"))
    assert_eq!(parse_version("HTTP/1.5"), Err("Bad status line: HTTP/1.5"))
    assert_eq!(parse_version(""), Err("Bad status line: "))
}

#[test]
fn test_parse_topline() {
    assert_eq!(parse_topline("HTTP/1.1 301 Moved"), Ok((11, 301, "Moved")))
    assert_eq!(parse_topline("HTTP/0.9 301 Moved Permanently"), Ok((9, 301, "Moved Permanently")))
}

#[test]
fn test_is_last() {
    assert_eq!(is_last(&"\r\n".to_string()), true)
    assert_eq!(is_last(&"\n".to_string()), true)
    assert_eq!(is_last(&"foo".to_string()), false)
    assert_eq!(is_last(&"\n ".to_string()), false)
    assert_eq!(is_last(&" \n".to_string()), false)
}
