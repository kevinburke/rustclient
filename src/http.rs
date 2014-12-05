extern crate url;
use std::time;
use std::result::{Result};
use std::io::net::ip;
use url::{Url, SchemeData, ParseError};
use dns;

fn domain_is_ipaddr(domain: &str) -> bool {
    return from_str::<ip::IpAddr>(domain).is_some()
}

pub fn get(raw_url: &str) -> bool {
    let url_r = Url::parse(raw_url);
    match url_r {
        Ok(ref url) => {
            let dom = "foobar"; 
            //match dom {
                //Some(d) => {
                    //if (domain_is_ipaddr(d)) {
                    //} else {
                        //let addrs = dns::lookup(d, time::Duration::seconds(30));
                        //println!("{}", addrs);
                    //}
                //}
                //None => { return false ; }
            //}
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
