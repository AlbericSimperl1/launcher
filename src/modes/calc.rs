//! Inline calculator: recursive-descent parser met functies, constanten en `ans`.

pub fn eval(input: &str, ans: Option<f64>) -> Result<f64, String> {
    let mut p = Parser {
        chars: input.chars().collect(),
        pos: 0,
        ans,
    };
    p.skip_ws();
    if p.eof() {
        return Err("invalid expression".into());
    }
    let v = p.expr()?;
    p.skip_ws();
    if !p.eof() {
        return Err("invalid expression".into());
    }
    if v.is_finite() {
        Ok(v)
    } else {
        Err("undefined result".into())
    }
}

pub fn format(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    if v.abs() >= 1e15 || v.abs() < 1e-9 {
        return format!("{v:e}");
    }
    let mut s = format!("{v:.10}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
    ans: Option<f64>,
}

impl Parser {
    fn eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn eat(&mut self, c: char) -> bool {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expr(&mut self) -> Result<f64, String> {
        let mut v = self.term()?;
        loop {
            if self.eat('+') {
                v += self.term()?;
            } else if self.eat('-') {
                v -= self.term()?;
            } else {
                return Ok(v);
            }
        }
    }

    fn term(&mut self) -> Result<f64, String> {
        let mut v = self.unary()?;
        loop {
            if self.eat('*') || self.eat('×') {
                v *= self.unary()?;
            } else if self.eat('/') || self.eat('÷') {
                let d = self.unary()?;
                if d == 0.0 {
                    return Err("division by zero".into());
                }
                v /= d;
            } else if self.eat('%') {
                let d = self.unary()?;
                if d == 0.0 {
                    return Err("division by zero".into());
                }
                v %= d;
            } else {
                return Ok(v);
            }
        }
    }

    fn unary(&mut self) -> Result<f64, String> {
        if self.eat('-') {
            Ok(-self.unary()?)
        } else if self.eat('+') {
            self.unary()
        } else {
            self.power()
        }
    }

    fn power(&mut self) -> Result<f64, String> {
        let base = self.atom()?;
        self.skip_ws();
        if self.eat('^') {
            let exp = self.unary()?;
            Ok(base.powf(exp))
        } else {
            Ok(base)
        }
    }

    fn atom(&mut self) -> Result<f64, String> {
        self.skip_ws();
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                let v = self.expr()?;
                if !self.eat(')') {
                    return Err("invalid expression".into());
                }
                Ok(v)
            }
            Some(c) if c.is_ascii_digit() || c == '.' => self.number(),
            Some(c) if c.is_alphabetic() => self.ident(),
            _ => Err("invalid expression".into()),
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == '.' || c == '_') {
            self.pos += 1;
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            let save = self.pos;
            self.pos += 1;
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.pos += 1;
            }
            if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    self.pos += 1;
                }
            } else {
                self.pos = save;
            }
        }
        let s: String = self.chars[start..self.pos].iter().filter(|c| **c != '_').collect();
        s.parse::<f64>().map_err(|_| "invalid expression".to_string())
    }

    fn ident(&mut self) -> Result<f64, String> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_alphanumeric() || c == '_') {
            self.pos += 1;
        }
        let name: String = self.chars[start..self.pos].iter().collect::<String>().to_lowercase();
        self.skip_ws();
        if self.peek() == Some('(') {
            self.pos += 1;
            let mut args = Vec::new();
            if !self.eat(')') {
                loop {
                    args.push(self.expr()?);
                    if self.eat(',') {
                        continue;
                    }
                    if self.eat(')') {
                        break;
                    }
                    return Err("invalid expression".into());
                }
            }
            return call(&name, &args);
        }
        match name.as_str() {
            "pi" | "π" => Ok(std::f64::consts::PI),
            "e" => Ok(std::f64::consts::E),
            "tau" => Ok(std::f64::consts::TAU),
            "ans" => self.ans.ok_or_else(|| "no previous result (ans)".to_string()),
            _ => Err(format!("unknown name: {name}")),
        }
    }
}

fn call(name: &str, a: &[f64]) -> Result<f64, String> {
    let one = |f: fn(f64) -> f64| -> Result<f64, String> {
        match a {
            [x] => Ok(f(*x)),
            _ => Err(format!("{name}() takes 1 argument")),
        }
    };
    match name {
        "sqrt" => one(f64::sqrt),
        "cbrt" => one(f64::cbrt),
        "sin" => one(f64::sin),
        "cos" => one(f64::cos),
        "tan" => one(f64::tan),
        "asin" => one(f64::asin),
        "acos" => one(f64::acos),
        "atan" => one(f64::atan),
        "sinh" => one(f64::sinh),
        "cosh" => one(f64::cosh),
        "tanh" => one(f64::tanh),
        "ln" => one(f64::ln),
        "log10" => one(f64::log10),
        "log2" => one(f64::log2),
        "exp" => one(f64::exp),
        "abs" => one(f64::abs),
        "floor" => one(f64::floor),
        "ceil" => one(f64::ceil),
        "round" => one(f64::round),
        "deg" => one(f64::to_degrees),
        "rad" => one(f64::to_radians),
        "log" => match a {
            [x] => Ok(x.log10()),
            [x, b] => Ok(x.log(*b)),
            _ => Err("log() takes 1 or 2 arguments".into()),
        },
        "pow" => match a {
            [x, y] => Ok(x.powf(*y)),
            _ => Err("pow() takes 2 arguments".into()),
        },
        "min" if !a.is_empty() => Ok(a.iter().cloned().fold(f64::INFINITY, f64::min)),
        "max" if !a.is_empty() => Ok(a.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
        _ => Err(format!("unknown function: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basics() {
        assert_eq!(format(eval("2+2", None).unwrap()), "4");
        assert_eq!(format(eval("ans*2", Some(4.0)).unwrap()), "8");
        assert_eq!(format(eval("2^3^2", None).unwrap()), "512");
        assert_eq!(format(eval("-2^2", None).unwrap()), "-4");
        assert_eq!(format(eval("sqrt(16)*pow(2,3)", None).unwrap()), "32");
        assert_eq!(format(eval("1/3", None).unwrap()), "0.3333333333");
        assert!(eval("2+", None).is_err());
        assert!(eval("1/0", None).is_err());
    }
}
