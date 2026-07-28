use super::{
    Element, MAX_BINARY_BYTES, MAX_COMPONENTS_PER_ELEMENT, MAX_ELEMENTS_PER_SEGMENT,
    MAX_MESSAGE_BYTES, MAX_SEGMENTS, SYNTAX_BYTES, Segment, Value, WireError, parse_decimal_usize,
};

pub(super) fn parse_segment_sequence(input: &[u8]) -> Result<Vec<Segment>, WireError> {
    if input.is_empty() || input.last() != Some(&b'\'') {
        return Err(WireError::MissingSegmentTerminator);
    }
    if input.len() > MAX_MESSAGE_BYTES {
        return Err(WireError::MessageTooLarge {
            limit: MAX_MESSAGE_BYTES,
        });
    }

    let mut parser = Parser::new(input);
    let mut segments = Vec::new();
    while !parser.is_finished() {
        if segments.len() == MAX_SEGMENTS {
            return Err(WireError::TooManySegments {
                limit: MAX_SEGMENTS,
            });
        }
        segments.push(parser.parse_segment(segments.len() + 1)?);
    }
    Ok(segments)
}

struct Parser<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn is_finished(&self) -> bool {
        self.offset == self.input.len()
    }

    fn parse_segment(&mut self, segment: usize) -> Result<Segment, WireError> {
        let mut elements = Vec::new();
        loop {
            if elements.len() == MAX_ELEMENTS_PER_SEGMENT {
                return Err(WireError::TooManyElements {
                    segment,
                    limit: MAX_ELEMENTS_PER_SEGMENT,
                });
            }
            elements.push(self.parse_element(segment, elements.len() + 1)?);
            match self.peek() {
                Some(b'+') => {
                    self.offset += 1;
                }
                Some(b'\'') => {
                    self.offset += 1;
                    break;
                }
                None => return Err(WireError::MissingSegmentTerminator),
                Some(_) => {
                    return Err(WireError::UnexpectedDelimiter {
                        offset: self.offset,
                    });
                }
            }
        }
        Ok(Segment { elements })
    }

    fn parse_element(&mut self, segment: usize, element: usize) -> Result<Element, WireError> {
        let mut components = Vec::new();
        loop {
            if components.len() == MAX_COMPONENTS_PER_ELEMENT {
                return Err(WireError::TooManyComponents {
                    segment,
                    element,
                    limit: MAX_COMPONENTS_PER_ELEMENT,
                });
            }
            components.push(self.parse_value()?);
            if self.peek() == Some(b':') {
                self.offset += 1;
            } else {
                break;
            }
        }
        Ok(Element { components })
    }

    fn parse_value(&mut self) -> Result<Value, WireError> {
        if self.peek() == Some(b'@') {
            return self.parse_binary();
        }

        let mut text = Vec::new();
        loop {
            match self.peek() {
                Some(b'+') | Some(b':') | Some(b'\'') | None => break,
                Some(b'?') => {
                    let escape_offset = self.offset;
                    self.offset += 1;
                    let escaped = self.peek().ok_or(WireError::IncompleteEscape {
                        offset: escape_offset,
                    })?;
                    if !SYNTAX_BYTES.contains(&escaped) {
                        return Err(WireError::InvalidEscape {
                            offset: escape_offset,
                        });
                    }
                    text.push(escaped);
                    self.offset += 1;
                }
                Some(b'@') => {
                    return Err(WireError::MisplacedBinaryMarker {
                        offset: self.offset,
                    });
                }
                Some(byte) => {
                    text.push(byte);
                    self.offset += 1;
                }
            }
        }
        Ok(Value::Text(text))
    }

    fn parse_binary(&mut self) -> Result<Value, WireError> {
        let marker_offset = self.offset;
        self.offset += 1;
        let length_start = self.offset;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.offset += 1;
        }
        if self.offset == length_start || self.peek() != Some(b'@') {
            return Err(WireError::InvalidBinaryLength {
                offset: marker_offset,
            });
        }
        let length = parse_decimal_usize(&self.input[length_start..self.offset]).ok_or(
            WireError::InvalidBinaryLength {
                offset: marker_offset,
            },
        )?;
        if length > MAX_BINARY_BYTES {
            return Err(WireError::BinaryTooLarge {
                offset: marker_offset,
                limit: MAX_BINARY_BYTES,
            });
        }

        self.offset += 1;
        let end = self
            .offset
            .checked_add(length)
            .filter(|end| *end <= self.input.len())
            .ok_or(WireError::TruncatedBinary {
                offset: marker_offset,
            })?;
        let binary = self.input[self.offset..end].to_vec();
        self.offset = end;
        if !matches!(self.peek(), Some(b'+') | Some(b':') | Some(b'\'') | None) {
            return Err(WireError::BinaryHasTrailingData {
                offset: marker_offset,
            });
        }
        Ok(Value::Binary(binary))
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.offset).copied()
    }
}
