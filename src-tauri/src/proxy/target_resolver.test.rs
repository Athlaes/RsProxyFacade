use super::*;

#[test]
fn matches_hosts_and_globs_without_crossing_domain_boundaries() {
    for (host, rules, expected) in [
        ("example.com", "", false),
        ("EXAMPLE.com.", "localhost; example.COM", true),
        ("evil-example.com", "example.com", false),
        ("a.example.com", "*.example.com", true),
        ("example.com", "*.example.com", false),
        ("example.com", ".example.com", true),
        ("a.example.com", ".example.com", true),
        ("notexample.com", ".example.com", false),
        ("example.com.evil", "*.example.com", false),
        ("api.dev.internal", "api.*.internal", true),
        ("anything.example", "*", true),
        ("127.0.0.1", "localhost, 127.*", true),
    ] {
        assert_eq!(matches_bypass(host, rules).unwrap(), expected, "{host}: {rules}");
    }
}

#[test]
fn local_means_simple_hostname_not_private_ip() {
    for (host, expected) in [
        ("printer", true),
        ("printer.", true),
        ("printer.example", false),
        ("127.0.0.1", false),
        ("[::1]", false),
    ] {
        assert_eq!(matches_bypass(host, "<LOCAL>").unwrap(), expected);
    }
}

#[test]
fn matches_literal_ips_and_cidr_without_dns() {
    for (host, rule, expected) in [
        ("10.1.2.3", "10.0.0.0/8", true),
        ("11.0.0.1", "10.0.0.0/8", false),
        ("10.1.2.3", "10.1.2.3/32", true),
        ("localhost", "127.0.0.0/8", false),
        ("[2001:db8::1]", "2001:db8::/32", true),
        ("[::1]", "0:0:0:0:0:0:0:1", true),
        ("::1", "[::1]", true),
        ("::1", "0.0.0.0/0", false),
    ] {
        assert_eq!(matches_bypass(host, rule).unwrap(), expected, "{host}: {rule}");
    }
}

#[test]
fn invalid_rules_fail_even_after_a_match() {
    for rule in [
        "10.0.0.0/33",
        "::/129",
        "https://example.com",
        "example.com:80",
        "<invalid>",
        "a?b",
        "a..b",
        "bad rule",
    ] {
        assert!(matches!(
            matches_bypass("example.com", &format!("*;{rule}")),
            Err(TargetResolveError::InvalidBypassRule { .. })
        ));
    }
}

#[test]
fn manual_resolution_applies_bypass_to_http_and_connect() {
    for uri in ["http://api.example/path", "api.example:443"] {
        let uri: Uri = uri.parse().unwrap();
        assert!(matches!(
            &resolve_manual(&uri, "proxy", &8080, "*.example").unwrap()[0],
            ResolvedDestination::Direct
        ));
        assert!(
            matches!(&resolve_manual(&uri, "proxy", &8080, "elsewhere.example").unwrap()[0], ResolvedDestination::HttpProxy { host, port: 8080 } if host == "proxy")
        );
        assert!(&resolve_manual(&uri, "proxy", &8080, "invalid/99").is_err());
    }
}
