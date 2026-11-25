use hey_i18n::{direction_of, format_date, format_number, Arg, Direction, I18n};

fn sample() -> I18n {
    let mut i = I18n::new("en");
    i.add_ftl(
        "en",
        "hello = Hello\ngreet = Hi, { $name }!\nonly-en = English only\nitems = { $n ->\n    [one] { $n } item\n   *[other] { $n } items\n}\n",
    )
    .unwrap();
    i.add_ftl("ru", "hello = Привет\ngreet = Привет, { $name }!\n").unwrap();
    i
}

#[test]
fn translates_with_fallback_chain() {
    let mut i = sample();
    assert_eq!(i.tr("hello"), "Hello");
    i.set_locale("ru-RU");
    assert_eq!(i.tr("hello"), "Привет"); // region falls back to the language bundle
    assert_eq!(i.tr("only-en"), "English only"); // missing in ru -> default locale
    assert_eq!(i.tr("nope"), "nope"); // missing everywhere -> key
    assert_eq!(i.tr_args("greet", &[("name", Arg::from("Vit"))]), "Привет, Vit!");
}

#[test]
fn plural_selectors_work() {
    let i = sample();
    assert_eq!(i.tr_args("items", &[("n", Arg::from(1i64))]), "1 item");
    assert_eq!(i.tr_args("items", &[("n", Arg::from(5i64))]), "5 items");
}

#[test]
fn rejects_bad_input() {
    let mut i = I18n::new("en");
    assert!(i.add_ftl("en", "broken = { $").is_err());
    assert!(i.add_ftl("not a locale!!", "a = b").is_err());
}

#[test]
fn numbers_and_dates_follow_locale() {
    assert_eq!(format_number("en-US", 1234567.891, 2), "1,234,567.89");
    assert_eq!(format_number("ru", 1234567.891, 2), "1\u{a0}234\u{a0}567,89");
    assert_eq!(format_number("de", -1234.5, 1), "-1.234,5");
    assert_eq!(format_number("en", -0.0001, 2), "0.00");
    assert_eq!(format_number("en", 999.0, 0), "999");
    assert_eq!(format_date("en-US", 2026, 3, 7), "03/07/2026");
    assert_eq!(format_date("en-GB", 2026, 3, 7), "07/03/2026");
    assert_eq!(format_date("ru", 2026, 3, 7), "07.03.2026");
    assert_eq!(format_date("ja", 2026, 3, 7), "2026-03-07");
}

#[test]
fn rtl_detection() {
    assert_eq!(direction_of("ar-EG"), Direction::Rtl);
    assert_eq!(direction_of("he"), Direction::Rtl);
    assert_eq!(direction_of("ru"), Direction::Ltr);
    let mut i = sample();
    i.set_locale("ar");
    assert_eq!(i.direction(), Direction::Rtl);
}
