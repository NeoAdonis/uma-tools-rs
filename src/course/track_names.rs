use crate::configuration::Language;

/// Get the track name for a given track ID and language.
/// Returns `None` if the track ID is unknown.
pub(crate) fn track_name(track_id: u16, language: Language) -> Option<&'static str> {
    match (track_id, language) {
        (10001, Language::Japanese) => Some("札幌"),
        (10001, _) => Some("Sapporo"),
        (10002, Language::Japanese) => Some("函館"),
        (10002, _) => Some("Hakodate"),
        (10003, Language::Japanese) => Some("新潟"),
        (10003, _) => Some("Niigata"),
        (10004, Language::Japanese) => Some("福島"),
        (10004, _) => Some("Fukushima"),
        (10005, Language::Japanese) => Some("中山"),
        (10005, _) => Some("Nakayama"),
        (10006, Language::Japanese) => Some("東京"),
        (10006, _) => Some("Tokyo"),
        (10007, Language::Japanese) => Some("中京"),
        (10007, _) => Some("Chuukyo"),
        (10008, Language::Japanese) => Some("京都"),
        (10008, _) => Some("Kyoto"),
        (10009, Language::Japanese) => Some("阪神"),
        (10009, _) => Some("Hanshin"),
        (10010, Language::Japanese) => Some("小倉"),
        (10010, _) => Some("Kokura"),
        (10101, Language::Japanese) => Some("大井"),
        (10101, _) => Some("Ooi"),
        (10103, Language::Japanese) => Some("川崎"),
        (10103, _) => Some("Kawasaki"),
        (10104, Language::Japanese) => Some("船橋"),
        (10104, _) => Some("Funabashi"),
        (10105, Language::Japanese) => Some("盛岡"),
        (10105, _) => Some("Morioka"),
        (10201, Language::Japanese) => Some("ﾛﾝｼｬﾝ"),
        (10201, _) => Some("Longchamp"),
        (10202, Language::Japanese) => Some("ｻﾝﾀｱﾆﾀﾊﾟｰｸ"),
        (10202, _) => Some("Santa Anita Park"),
        (10203, Language::Japanese) => Some("ﾃﾞﾙﾏｰ"),
        (10203, _) => Some("Del Mar"),
        _ => None,
    }
}
