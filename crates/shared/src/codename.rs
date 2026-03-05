use rand::Rng;

const ADJECTIVES: &[&str] = &[
    "amber", "bold", "calm", "dark", "eager", "fast", "glad", "hazy", "iron", "jade", "keen",
    "lush", "mild", "neat", "opal", "pale", "quiet", "rare", "sage", "teal", "ultra", "vast",
    "warm", "zinc", "aqua", "blue", "crisp", "deep", "fair", "gold", "hush", "icy", "just", "kind",
    "lean", "moss", "navy", "oak", "pure", "quick", "rosy", "slim", "true", "used", "vine", "wide",
    "young", "zeal", "ashen", "brave", "clear", "dusty", "elite", "fresh", "green", "happy",
    "ivory", "jolly", "lunar", "maple", "noble", "olive", "prime", "royal", "solar", "terra",
    "urban", "vivid", "windy", "xenon", "misty", "coral",
];

const NOUNS: &[&str] = &[
    "ant", "bat", "cat", "dove", "elk", "fox", "goat", "hare", "ibis", "jay", "koi", "lark",
    "moth", "newt", "owl", "puma", "quail", "ram", "seal", "toad", "urchin", "vole", "wasp", "yak",
    "ape", "bass", "crab", "deer", "eel", "frog", "gull", "hawk", "iguana", "jackal", "koala",
    "lion", "mule", "narwhal", "otter", "parrot", "raven", "shark", "tiger", "tern", "viper",
    "whale", "wren", "zebra", "bear", "crane", "duck", "eagle", "finch", "gecko", "horse", "imp",
    "jaguar", "kite", "lynx", "mink", "osprey", "panda", "robin", "swan", "trout", "unicorn",
    "walrus", "wolf", "falcon", "heron", "lemur", "moose",
];

/// Generate a friendly codename like "iron-falcon" or "calm-otter".
pub fn generate_codename() -> String {
    let mut rng = rand::thread_rng();
    let adj = ADJECTIVES[rng.gen_range(0..ADJECTIVES.len())];
    let noun = NOUNS[rng.gen_range(0..NOUNS.len())];
    format!("{adj}-{noun}")
}

/// Generate a codename that doesn't collide with existing ones.
pub fn generate_unique_codename(existing: &[String]) -> String {
    for _ in 0..100 {
        let name = generate_codename();
        if !existing.contains(&name) {
            return name;
        }
    }
    // Fallback: append a number
    let base = generate_codename();
    let mut rng = rand::thread_rng();
    format!("{base}-{}", rng.gen_range(100..999))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codename_format() {
        let name = generate_codename();
        assert!(name.contains('-'), "codename should contain a dash: {name}");
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(
            parts.len(),
            2,
            "codename should have exactly two parts: {name}"
        );
    }

    #[test]
    fn unique_codename_avoids_collisions() {
        let existing = vec!["iron-falcon".into()];
        let name = generate_unique_codename(&existing);
        assert_ne!(name, "iron-falcon");
    }
}
