struct KotlinParser {
    classes: Vec<Class>,
}

struct Class {
    fields: Vec<Field>,
    package: String,
}

struct Field {
    field_type: String,
}

impl KotlinParser {
    pub fn new() -> Self {
        KotlinParser { classes: vec![] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_kotlin_parser() {
        let parser = KotlinParser::new();
        assert!(parser.classes.is_empty());
    }
}
