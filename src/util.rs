use tabled::settings::{
    style::{HorizontalLine, On},
    Settings, Style,
};

pub fn table_settings() -> Settings<Settings, Style<On, On, On, On, (), On, [HorizontalLine; 1]>> {
    Settings::default().with(Style::rounded())
}
