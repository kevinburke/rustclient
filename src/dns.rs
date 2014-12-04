use std::comm;
use std::io;
use std::time;
use std::io::timer;
use std::io::net::addrinfo;
use std::io::net::ip;

fn lookup(host: &'static str, timeout_duration: time::Duration) -> io::IoResult<Vec<ip::IpAddr>> {
    let (tx, rx): (Sender<io::IoResult<Vec<ip::IpAddr>>>, Receiver<io::IoResult<Vec<ip::IpAddr>>>) = comm::channel();
    let mut t = timer::Timer::new().unwrap();
    let timeout = t.oneshot(timeout_duration);

    let detail = format!("Failed to resolve {} after {} milliseconds", 
                         host, timeout_duration.num_milliseconds());

    spawn(proc() {
        tx.send(addrinfo::get_host_addresses(host));
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

fn main() {
    //let addrs = addrinfo::get_host_addresses("api.twilio.com");
    let lresult = lookup("api.twilio.com", time::Duration::seconds(3));
    match lresult {
        Ok(addrs) => {
            println!("{}", addrs)
        },
        Err(why) => println!("{}", why),
    }
}
