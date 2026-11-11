//! Java 相关工具。解析统一走 `parse`，生成器各自独立。

mod assign;
mod json_pojo;
pub(crate) mod parse;
mod pojo_json;

pub use assign::{DtoVo, SetterChain};
pub use json_pojo::JsonToPojo;
pub use pojo_json::PojoToJson;
