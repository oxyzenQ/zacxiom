mod tests {
    use super::*;

    #[test]
    fn test_rule_priority_system_first() {
        let rules = rule_database();
        // First rules should be system protection
        assert_eq!(rules[0].name, "sys-bin-usr");
        assert_eq!(rules[1].name, "sys-bin-root");
        assert_eq!(rules[2].name, "sys-etc");
    }

    #[test]
    fn test_system_binary_not_cache() {
        let rules = rule_database();
        let brave_rule = rules.iter().find(|r| r.name == "cache-browser").unwrap();
        // brave binary path should NOT match browser cache rule
        assert!(!(brave_rule.matches)(
            Path::new("/usr/bin/brave"),
            &"/usr/bin/brave".to_lowercase()
        ));
        // But brave cache SHOULD match
        assert!((brave_rule.matches)(
            Path::new("/home/user/.cache/BraveSoftware/Brave-Browser/Cache/data_0"),
            &"/home/user/.cache/bravesoftware/brave-browser/cache/data_0".to_lowercase()
        ));
    }

    #[test]
    fn test_etc_never_cache() {
        let rules = rule_database();
        let cache_rule = rules.iter().find(|r| r.name == "cache-user").unwrap();
        assert!(!(cache_rule.matches)(
            Path::new("/etc/environment"),
            &"/etc/environment".to_lowercase()
        ));
    }

    #[test]
    fn test_home_root_not_cleanable() {
        let rules = rule_database();
        let home_rule = rules.iter().find(|r| r.name == "home-root").unwrap();
        assert!((home_rule.matches)(
            Path::new("/home/user"),
            &"/home/user".to_lowercase()
        ));
        assert!(!(home_rule.matches)(
            Path::new("/home/user/.cache"),
            &"/home/user/.cache".to_lowercase()
        ));
    }

    #[test]
    fn test_ssh_is_protected() {
        let rules = rule_database();
        let ssh_rule = rules.iter().find(|r| r.name == "sec-ssh").unwrap();
        assert!((ssh_rule.matches)(
            Path::new("/home/user/.ssh/id_ed25519"),
            &"/home/user/.ssh/id_ed25519".to_lowercase()
        ));
        assert_eq!(ssh_rule.category, Category::SecurityCredential);
        assert_eq!(ssh_rule.risk_level, RiskLevel::Critical);
    }
}
