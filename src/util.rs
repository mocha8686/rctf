use tabled::{
    settings::{
        style::{HorizontalLine, On},
        Settings, Style,
    },
    Table, Tabled,
};

pub(crate) fn base_table_settings(
) -> Settings<Settings, Style<On, On, On, On, (), On, [HorizontalLine; 1]>> {
    Settings::default().with(Style::rounded())
}

pub(crate) fn base_table<T, I>(items: I) -> Table
where
    I: IntoIterator<Item = T>,
    T: Tabled,
{
    let mut table = Table::new(items);
    table.with(base_table_settings());
    table
}
