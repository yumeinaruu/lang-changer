/// Characters typed in the current word, as the user sees them.
#[derive(Default)]
pub struct WordBuffer {
    chars: String,
}

impl WordBuffer {
    pub fn new() -> WordBuffer {
        WordBuffer::default()
    }

    pub fn push(&mut self, ch: char) {
        self.chars.push(ch);
    }

    pub fn pop(&mut self) {
        self.chars.pop();
    }

    pub fn reset(&mut self) {
        self.chars.clear();
    }

    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.chars)
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_chars() {
        let mut b = WordBuffer::new();
        b.push('g');
        b.push('h');
        assert!(!b.is_empty());
        assert_eq!(b.take(), "gh");
    }

    #[test]
    fn backspace_removes_last() {
        let mut b = WordBuffer::new();
        b.push('a');
        b.push('b');
        b.pop();
        assert_eq!(b.take(), "a");
        b.pop(); // popping an empty buffer must not panic
        assert!(b.is_empty());
    }

    #[test]
    fn reset_clears() {
        let mut b = WordBuffer::new();
        b.push('a');
        b.reset();
        assert!(b.is_empty());
    }

    #[test]
    fn take_returns_and_clears() {
        let mut b = WordBuffer::new();
        b.push('п');
        assert_eq!(b.take(), "п");
        assert!(b.is_empty());
        assert_eq!(b.take(), "");
    }
}
