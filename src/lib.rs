extern crate url;

use std::comm;
use std::io;
use std::time;
use std::result::{Result};
use std::io::timer;
use std::io::net::ip;
use std::io::net::addrinfo;

use url::{Url, SchemeData, ParseError};

/// Find a list of IP addresses for the given host. Give up after
/// `timeout_duration` if the DNS server does not return a response.
pub fn lookup(host: &str, timeout_duration: time::Duration) -> io::IoResult<Vec<ip::IpAddr>> {
    let ownedhost = host.into_string();
    let (tx, rx): (Sender<io::IoResult<Vec<ip::IpAddr>>>, Receiver<io::IoResult<Vec<ip::IpAddr>>>) = comm::channel();
    let mut t = timer::Timer::new().unwrap();
    let timeout = t.oneshot(timeout_duration);

    let detail = format!("Failed to resolve {} after {} milliseconds", 
                         ownedhost.as_slice(), timeout_duration.num_milliseconds());

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

/// Make a HTTP request and return a response (eventually)
pub fn get(raw_url: &str) -> bool {
    let parsed_url = Url::parse(raw_url);
    match parsed_url {
        Ok(url) => {
            match url.domain() {
                Some(d) => {
                    let staticd = d;
                    let addrs = lookup(staticd, time::Duration::seconds(30));
                    println!("{}", addrs);
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
    let out = get("https://api.twilio.com");
    assert!(false)
}

#[test]
fn test_domain() {
    assert!(domain_is_ipaddr("10.0.0.1"));
    assert!(domain_is_ipaddr("::1"));
    assert!(!domain_is_ipaddr("api.twilio.com"));
    assert!(!domain_is_ipaddr("foo"));
}
