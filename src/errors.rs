error_chain! {
    errors {
        HandlerNotFound
        UnknownError
    }
    foreign_links {
        Url(::url::ParseError);
        Io(::std::io::Error);
        Toml(::toml::de::Error);
        Env(::std::env::VarError);
        Regex(::regex::Error);
    }
}