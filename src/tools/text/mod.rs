//! 文本类工具：乱码修复、字数统计、拼音首字母。

mod count;
mod mojibake;
mod pinyin;

pub use count::Count;
pub use mojibake::Mojibake;
pub use pinyin::Pinyin;
