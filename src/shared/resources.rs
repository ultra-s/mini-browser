pub const IDI_CEFSIMPLE: u16 = 120;
pub const IDI_SMALL: u16 = 121;

/// The Mini start page ships embedded in the binary.
pub fn start_page_html(_stealth: bool) -> &'static str {
    include_str!("startpage.html")
}
