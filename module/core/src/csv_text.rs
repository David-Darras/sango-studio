//! The rules of CSV for the texts: a value with a comma, a quote or a line break is in quotes, and a quote inside the
//! quotes is written twice.

/// Writes a value with quotes when it needs them.
pub fn quote(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Cuts CSV text into records of fields. A field in quotes can have commas, line breaks and quotes (written twice).
/// Gives each record with the number of the line where it starts.
pub fn records(text: &str) -> Result<Vec<(usize, Vec<String>)>, String> {
    let mut out = Vec::new();
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    let mut line = 1;
    let mut start = 1;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                _ => {
                    if c == '\n' {
                        line += 1;
                    }
                    field.push(c);
                }
            }
            continue;
        }
        match c {
            '"' if field.is_empty() && !was_quoted => {
                quoted = true;
                was_quoted = true;
            }
            '"' => return Err(format!("Line {line}: a quote is in the middle of a field.")),
            ',' => {
                fields.push(std::mem::take(&mut field));
                was_quoted = false;
            }
            '\r' => {}
            '\n' => {
                fields.push(std::mem::take(&mut field));
                was_quoted = false;
                out.push((start, std::mem::take(&mut fields)));
                line += 1;
                start = line;
            }
            _ => {
                if was_quoted {
                    return Err(format!("Line {line}: text after the closing quote."));
                }
                field.push(c);
            }
        }
    }
    if quoted {
        return Err(format!("Line {start}: a quote is not closed."));
    }
    if !field.is_empty() || !fields.is_empty() || was_quoted {
        fields.push(field);
        out.push((start, fields));
    }
    Ok(out)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_quotes_and_the_line_breaks_are_kept() {
        let text = "a,\"b,c\",\"d \"\"e\"\"
f\",g
h,i,j,k,l
";
        let found = records(text).unwrap();
        assert_eq!(found[0], (1, vec!["a".to_string(), "b,c".to_string(), "d \"e\"
f".to_string(), "g".to_string()]));
        assert_eq!(found[1].0, 3);
        assert_eq!(found[1].1.len(), 5);
        assert_eq!(quote("d \"e\"
f"), "\"d \"\"e\"\"
f\"");
        assert_eq!(quote("plain"), "plain");
        assert!(records("a,\"b").is_err());
        assert!(records("a,b\"c").is_err());
    }
}
