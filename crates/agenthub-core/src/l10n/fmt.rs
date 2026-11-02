use std::borrow::Cow;

/// `{name}` 占位符替换。
///
/// 参数里没有的占位符原样保留：模板里真的写了 `{` 不会被吃掉，拼错的参数名
/// 也能在输出里直接看见，而不是静默变成空串。
pub fn interpolate<'a>(template: &'a str, args: &[(&str, &str)]) -> Cow<'a, str> {
    if args.is_empty() || !template.contains('{') {
        return Cow::Borrowed(template);
    }
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return Cow::Owned(out);
        };
        let name = &after[..end];
        match args.iter().find(|(key, _)| *key == name) {
            Some((_, value)) => out.push_str(value),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Cow::Owned(out)
}
