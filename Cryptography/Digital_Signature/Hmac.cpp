// HMAC-SHA256 (RFC 2104) with SHA-256 (FIPS 180-4) from scratch.
// No external deps. C++17. Single file.
//
// Build:  g++ -std=c++17 -O2 -Wall -Wextra -o hmac Hmac.cpp
// Usage:
//   ./hmac                                  -> run RFC 4231 self-tests
//   ./hmac <key> <message>                   -> HMAC-SHA256 hex (raw ASCII bytes)
//   ./hmac --hex <key_hex> <message_ascii>   -> hex key, ASCII message
//   echo -n "msg" | ./hmac <key> -           -> read message from stdin ("-" token)
//
// Interop check:
//   printf 'Hi There' | openssl dgst -sha256 -hmac "$(printf '\x0b%.0s' {1..20}; echo)" -hex
//   python3 -c "import hmac,hashlib; print(hmac.new(b'\x0b'*20, b'Hi There', hashlib.sha256).hexdigest())"

#include <array>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <string>
#include <vector>

namespace crypto {

namespace detail {

//SHA defined Right Rotation 
inline uint32_t rotr(uint32_t x, unsigned n) { return (x >> n) | (x << (32 - n)); }

// SHA-256 round constants (fixed by spec, not secret)
constexpr std::array<uint32_t, 64> K = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
    0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
    0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
    0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
    0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
    0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2};

}  // namespace detail

// SHA-256 over [data, data+len). Returns 32-byte digest.
/**             
            512-bit block
                  │
                  ▼
          ┌───────────────┐
          │ Expand block  │
          │ into W[0..63] │
          └───────┬───────┘
                  │
                  ▼
        ┌────────────────────┐
        │ 64 SHA-256 rounds  │
        │                    │
        │ round 0 → K[0]     │
        │ round 1 → K[1]     │
        │ ...                │
        │ round 63 → K[63]   │
        └─────────┬──────────┘
                  │
                  ▼
            update h[0..7]
                  │
                  ▼
             256-bit hash
    */
std::array<uint8_t, 32> sha256(const uint8_t* data, size_t len) {
    using detail::K;
    using detail::rotr;

    uint32_t h[8] = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                     0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};

    // Padding: 0x80, zeros, then 64-bit big-endian bit length.
    // Total length must be a multiple of 64.
    uint64_t bit_len = static_cast<uint64_t>(len) * 8u;
    size_t padded = len + 1 + 8;
    if (padded % 64 != 0) padded += 64 - (padded % 64);

    std::vector<uint8_t> msg(padded, 0);
    if (len) std::memcpy(msg.data(), data, len);
    msg[len] = 0x80;
    for (int i = 0; i < 8; ++i)
        msg[padded - 8 + i] = static_cast<uint8_t>(bit_len >> (56 - 8 * i));

    for (size_t off = 0; off < padded; off += 64) {
        uint32_t w[64];
        for (int i = 0; i < 16; ++i) {
            w[i] = (static_cast<uint32_t>(msg[off + 4 * i]) << 24) |
                   (static_cast<uint32_t>(msg[off + 4 * i + 1]) << 16) |
                   (static_cast<uint32_t>(msg[off + 4 * i + 2]) << 8) |
                   (static_cast<uint32_t>(msg[off + 4 * i + 3]));
        }
        for (int i = 16; i < 64; ++i) {
            // SHA defined formulas
            uint32_t s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
            uint32_t s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }
        uint32_t a = h[0], b = h[1], c = h[2], d = h[3];
        uint32_t e = h[4], f = h[5], g = h[6], hh = h[7];
        for (int i = 0; i < 64; ++i) {
            uint32_t S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
            uint32_t ch = (e & f) ^ (~e & g);
            uint32_t t1 = hh + S1 + ch + K[i] + w[i];
            uint32_t S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
            uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
            uint32_t t2 = S0 + maj;
            hh = g; g = f; f = e; e = d + t1;
            d = c; c = b; b = a; a = t1 + t2;
        }
        h[0] += a; h[1] += b; h[2] += c; h[3] += d;
        h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
    }

    std::array<uint8_t, 32> out{};
    for (int i = 0; i < 8; ++i) {
        out[4 * i] = static_cast<uint8_t>(h[i] >> 24);
        out[4 * i + 1] = static_cast<uint8_t>(h[i] >> 16);
        out[4 * i + 2] = static_cast<uint8_t>(h[i] >> 8);
        out[4 * i + 3] = static_cast<uint8_t>(h[i]);
    }
    return out;
}

inline std::array<uint8_t, 32> sha256(const std::vector<uint8_t>& v) {
    return sha256(v.data(), v.size());
}

// HMAC-SHA256 per RFC 2104: H((K' ^ opad) || H((K' ^ ipad) || m)).
// K' = Hash(K) if len(K) > 64, else K zero-padded to 64 bytes.
/**                        
                        SECRET KEY K
                            │
                            ▼
                    ┌──────────────────┐
                    │   NORMALIZE K    │
                    │                  │
                    │ K > 64 bytes?    │
                    │      │           │
                    │   yes│ → SHA256  │
                    │      │           │
                    │ K ≤ 64 → zero pad│
                    └────────┬─────────┘
                             │
                             ▼
                    K' = exactly 64 bytes
                             │
               ┌─────────────┴─────────────┐
               │                           │
               │ XOR 0x36                  │ XOR 0x5c
               ▼                           ▼
           K' ⊕ ipad                    K' ⊕ opad
               │                           │
               │                           │
               │                     64-byte block
               │                           │
               ▼                           │
       ┌──────────────┐                    │
       │   MESSAGE    │                    │
       │      m       │                    │
       └──────┬───────┘                    │
              │                            │
              ▼                            │
       ┌────────────────────┐              │
       │ SHA256(            │              │
       │   (K'⊕ipad) || m   │              │
       │ )                  │              │
       └─────────┬──────────┘              │
                 │                         │
                 ▼                         │
             INNER HASH                    │
               32 bytes                    │
                 │                         │
                 └────────────┬────────────┘
                              ▼
                    ┌──────────────────┐
                    │   concatenate    │
                    │                  │
                    │ (K'⊕opad) ||     │
                    │    INNER_HASH    │
                    └────────┬─────────┘
                             │
                             ▼
                       ┌───────────┐
                       │ SHA-256   │
                       └─────┬─────┘
                             │
                             ▼
                      HMAC-SHA256
                         32 bytes
                         = 256 bits
 */
std::array<uint8_t, 32> hmac_sha256(const uint8_t* key, size_t key_len,
                                    const uint8_t* msg, size_t msg_len) {
    constexpr size_t kBlock = 64;
    std::array<uint8_t, kBlock> kb{};
    if (key_len > kBlock) {
        auto hk = sha256(key, key_len);
        std::memcpy(kb.data(), hk.data(), hk.size());
    } else if (key_len) {
        std::memcpy(kb.data(), key, key_len);
    }

    std::array<uint8_t, kBlock> ipad{}, opad{};
    for (size_t i = 0; i < kBlock; ++i) {
        ipad[i] = kb[i] ^ 0x36;
        opad[i] = kb[i] ^ 0x5c;
    }

    // inner = SHA256(ipad || msg)
    std::vector<uint8_t> inner;
    inner.reserve(kBlock + msg_len);
    inner.insert(inner.end(), ipad.begin(), ipad.end());
    inner.insert(inner.end(), msg, msg + msg_len);
    auto ih = sha256(inner);

    // outer = SHA256(opad || inner)
    std::vector<uint8_t> outer;
    outer.reserve(kBlock + ih.size());
    outer.insert(outer.end(), opad.begin(), opad.end());
    outer.insert(outer.end(), ih.begin(), ih.end());
    return sha256(outer);
}

inline std::array<uint8_t, 32> hmac_sha256(const std::vector<uint8_t>& key,
                                           const std::vector<uint8_t>& msg) {
    return hmac_sha256(key.data(), key.size(), msg.data(), msg.size());
}

// Constant-time compare (defense against timing side-channel on verify).
bool secure_equal(const std::array<uint8_t, 32>& a, const std::array<uint8_t, 32>& b) {
    volatile uint8_t d = 0;
    for (size_t i = 0; i < 32; ++i) d |= (a[i] ^ b[i]);
    return d == 0;
}

inline bool hmac_verify(const uint8_t* key, size_t key_len, const uint8_t* msg, size_t msg_len,
                        const std::array<uint8_t, 32>& expected) {
    return secure_equal(hmac_sha256(key, key_len, msg, msg_len), expected);
}

}  // namespace crypto

// ---- helpers (non-crypto) ----
static std::vector<uint8_t> str_bytes(const std::string& s) {
    return std::vector<uint8_t>(s.begin(), s.end());
}

static std::string to_hex(const uint8_t* p, size_t n) {
    std::ostringstream o;
    o << std::hex << std::setfill('0');
    for (size_t i = 0; i < n; ++i) o << std::setw(2) << static_cast<int>(p[i]);
    return o.str();
}

template <size_t N>
static std::string to_hex(const std::array<uint8_t, N>& a) {
    return to_hex(a.data(), a.size());
}

static std::vector<uint8_t> from_hex(const std::string& h) {
    if (h.size() % 2 != 0) throw std::runtime_error("odd-length hex");
    std::vector<uint8_t> out;
    out.reserve(h.size() / 2);
    auto nib = [](char c) -> int {
        if (c >= '0' && c <= '9') return c - '0';
        if (c >= 'a' && c <= 'f') return c - 'a' + 10;
        if (c >= 'A' && c <= 'F') return c - 'A' + 10;
        throw std::runtime_error("bad hex char");
    };
    for (size_t i = 0; i < h.size(); i += 2)
        out.push_back(static_cast<uint8_t>((nib(h[i]) << 4) | nib(h[i + 1])));
    return out;
}

static std::string read_all_stdin() {
    std::string s, chunk;
    char buf[8192];
    size_t n;
    while ((n = std::fread(buf, 1, sizeof buf, stdin)) > 0) s.append(buf, n);
    return s;
}

// ---- RFC 4231 self-tests ----
static int run_selftests() {
    struct Case {
        const char* name;
        std::vector<uint8_t> key, data;
        const char* want_hex;
    };
    auto rep = [](uint8_t b, int n) { return std::vector<uint8_t>(n, b); };
    std::vector<Case> cases = {
        // TC1: key = 0x0b x20, data = "Hi There"
        {"RFC4231-TC1", rep(0x0b, 20), str_bytes("Hi There"),
         "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"},
        // TC2: key = "Jefe"
        {"RFC4231-TC2", str_bytes("Jefe"), str_bytes("what do ya want for nothing?"),
         "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"},
        // TC3: key = 0xaa x20, data = 0xdd x50
        {"RFC4231-TC3", rep(0xaa, 20), rep(0xdd, 50),
         "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"},
        // TC4: key = 0x01..0x19 (25 bytes), data = 0xcd x50
        {"RFC4231-TC4",
         [] { std::vector<uint8_t> k(25); for (int i = 0; i < 25; ++i) k[i] = i + 1; return k; }(),
         rep(0xcd, 50),
         "82558a389a443c0ea4cc819899f2083a85f0faa3e578f8077a2e3ff46729665b"},
        // TC5 full (non-truncated): key = 0x0c x20
        {"RFC4231-TC5", rep(0x0c, 20), str_bytes("Test With Truncation"),
         "a3b6167473100ee06e0c796c2955552bfa6f7c0a6a8aef8b93f860aab0cd20c5"},
        // TC6: key = 0xaa x131 (> block size -> hashed first)
        {"RFC4231-TC6", rep(0xaa, 131),
         str_bytes("Test Using Larger Than Block-Size Key - Hash Key First"),
         "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"},
        // TC7: key = 0xaa x131, large data
        {"RFC4231-TC7", rep(0xaa, 131),
         str_bytes("Test Using Larger Than Block-Size Key and Larger Than One Block-Size Data"),
         "c9731f25665706dab8200d9ce68fad2cbac48efc4a5f72292e4eeb81e7d29298"},
    };

    int fails = 0;
    // SHA-256 sanity first (FIPS 180-4): "" and "abc".
    auto h_empty = crypto::sha256(nullptr, 0);
    auto h_abc = crypto::sha256(reinterpret_cast<const uint8_t*>("abc"), 3);
    auto check = [&](const char* n, const std::string& got, const char* want) {
        bool ok = (got == want);
        std::cout << (ok ? "[PASS] " : "[FAIL] ") << n << "\n";
        if (!ok) {
            std::cout << "  got:  " << got << "\n  want: " << want << "\n";
            ++fails;
        }
    };
    check("SHA256(\"\")", to_hex(h_empty),
          "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    check("SHA256(\"abc\")", to_hex(h_abc),
          "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

    for (auto& c : cases) {
        auto got = crypto::hmac_sha256(c.key, c.data);
        check(c.name, to_hex(got), c.want_hex);
        // verify path (constant-time) must accept its own output
        if (!crypto::hmac_verify(c.key.data(), c.key.size(), c.data.data(), c.data.size(), got)) {
            std::cout << "[FAIL] " << c.name << " verify() rejected valid tag\n";
            ++fails;
        }
    }
    // verify must reject a flipped tag
    {
        auto good = crypto::hmac_sha256(cases[0].key, cases[0].data);
        auto bad = good; bad[31] ^= 0x01;
        if (crypto::hmac_verify(cases[0].key.data(), cases[0].key.size(),
                                cases[0].data.data(), cases[0].data.size(), bad)) {
            std::cout << "[FAIL] verify() accepted forged tag\n";
            ++fails;
        } else {
            std::cout << "[PASS] tamper-rejection\n";
        }
    }
    std::cout << (fails ? "\nSELF-TEST FAILED\n" : "\nALL SELF-TESTS PASSED\n");
    return fails ? 1 : 0;
}

int main(int argc, char** argv) {
    if (argc == 1) return run_selftests();

    std::string mode = argv[1];
    if (mode == "--selftest" || mode == "test") return run_selftests();

    // ./hmac <key> <message>  |  ./hmac --hex <key_hex> <message>  |  "-" reads stdin
    bool hex_key = false;
    int ai = 1;
    if (mode == "--hex") {
        hex_key = true;
        ++ai;
    }
    if (argc - ai != 2) {
        std::cerr << "usage: " << argv[0] << " [--hex] <key> <message|->\n";
        return 2;
    }
    std::vector<uint8_t> key =
        hex_key ? from_hex(argv[ai]) : str_bytes(argv[ai]);
    std::string msgs = argv[ai + 1];
    std::vector<uint8_t> msg =
        (msgs == "-") ? str_bytes(read_all_stdin()) : str_bytes(msgs);

    auto tag = crypto::hmac_sha256(key, msg);
    std::cout << to_hex(tag) << "\n";
    return 0;
}
