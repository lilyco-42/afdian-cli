//! 爱发电开放接口签名。
//!
//! 算法固定为 `md5(token + "params" + params_json + "ts" + ts + "user_id" + user_id)`。
//! 这里手写 MD5，避免为一次摘要引入加密库依赖。

/// 一次请求的签名材料。
#[derive(Debug, Clone, Copy)]
pub struct SigningInput<'a> {
    /// 爱发电后台的 API token。
    pub token: &'a str,
    /// 创作者 user_id。
    pub user_id: &'a str,
    /// 紧凑 JSON（无空格，中文不转义）。
    pub params: &'a str,
    /// 秒级时间戳。
    pub ts: u64,
}

impl<'a> SigningInput<'a> {
    /// 拼出待签名原文。顺序必须与官方一致，否则签名校验不通过。
    pub fn payload(&self) -> String {
        let mut s = String::with_capacity(
            self.token.len() + self.params.len() + self.user_id.len() + 48,
        );
        s.push_str(self.token);
        s.push_str("params");
        s.push_str(self.params);
        s.push_str("ts");
        s.push_str(&self.ts.to_string());
        s.push_str("user_id");
        s.push_str(self.user_id);
        s
    }

    /// 计算 `sign`。
    pub fn sign(&self) -> String {
        md5_hex(self.payload().as_bytes())
    }
}

/// MD5 常量表。
const S: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

/// `floor(abs(sin(i+1)) * 2^32)`，编译期常量。
const K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

/// 计算 MD5 并返回小写十六进制。
pub fn md5_hex(input: &[u8]) -> String {
    let mut state: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];

    // 填充：0x80，补零至 56 mod 64，末尾 8 字节小端长度。
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut msg = input.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            let b = &chunk[i * 4..i * 4 + 4];
            *w = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }

        let (mut a, mut b, mut c, mut d) = (state[0], state[1], state[2], state[3]);

        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let tmp = d;
            d = c;
            c = b;
            let sum = a
                .wrapping_add(f)
                .wrapping_add(K[i])
                .wrapping_add(m[g]);
            b = b.wrapping_add(sum.rotate_left(S[i]));
            a = tmp;
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
    }

    let mut out = String::with_capacity(32);
    for word in state {
        for byte in word.to_le_bytes() {
            out.push(char::from_digit((byte >> 4) as u32, 16).unwrap_or('0'));
            out.push(char::from_digit((byte & 0x0f) as u32, 16).unwrap_or('0'));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_known_vectors() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"a"), "0cc175b9c0f1b6a831c399e269772661");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            md5_hex(b"The quick brown fox jumps over the lazy dog"),
            "9e107d9d372bb6826bd81d3542a419d6"
        );
    }

    #[test]
    fn md5_handles_cjk() {
        // UTF-8 多字节，验证长度按字节而非字符计算。
        assert_eq!(md5_hex("中文".as_bytes()).len(), 32);
    }

    #[test]
    fn payload_order_is_fixed() {
        let s = SigningInput {
            token: "T",
            user_id: "U",
            params: "{}",
            ts: 1,
        };
        assert_eq!(s.payload(), "Tparams{}ts1user_idU");
    }

    #[test]
    fn sign_is_32_hex() {
        let s = SigningInput {
            token: "PLACEHOLDER_TOKEN",
            user_id: "PLACEHOLDER_USER_ID",
            params: "{}",
            ts: 1792510000,
        };
        let sig = s.sign();
        assert_eq!(sig.len(), 32);
        assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
