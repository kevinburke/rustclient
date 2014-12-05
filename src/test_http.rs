use super::http;

#[test]
fn test_url() {
    let out = http::get("https://api.twilio.com");
    assert_eq!(out, "api.twilio.com");
}
