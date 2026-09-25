// RSA-2048 (RFC 8017 PKCS #1 v2.2) — keygen, raw trapdoor, PKCS#1 v1.5,
// OAEP-SHA256, and RSASSA-PKCS1-v1_5-SHA256 sign/verify.
//
// Bignum arithmetic delegates to libcrypto (BIGNUM); every RSA step
// itself — prime selection, e/d derivation, CRT, padding, DigestInfo —
// is implemented below, not via the high-level RSA_* API.
//
// Code structure: 
// 1. BIGNUM Library
// 2. Mathematical RSA
// 3. Padding Schemes
// 4. Encryption / Signature APIs
// 5. Tests

// Build (Apple Silicon, Homebrew OpenSSL 3):
//   g++ -std=c++17 -O2 -Wall -Wextra -o rsa RSA.cpp \
//       -I/opt/homebrew/opt/openssl@3/include \
//       -L/opt/homebrew/opt/openssl@3/lib -lcrypto
// Usage:
//   ./rsa                       -> self-tests (generates a fresh 2048-bit key)
//   ./rsa keygen [bits]         -> labeled hex: n e d p q dp dq qinv
//   ./rsa raw-enc <n> <e> <m>   -> hex ciphertext (all hex)
//   ./rsa raw-dec <n> <d> <c>   -> hex plaintext
//
// Interop (raw, no padding):
//   python3 -c "print(pow(65,17,3233))"            # == 2790 (textbook KAT below)
//   ./rsa raw-enc c81... <e> <m>  <->  python3 pow(m,e,n)

#include <openssl/bn.h>
#include <openssl/crypto.h>
#include <openssl/rand.h>
#include <openssl/sha.h>

#include <chrono>
#include <cstdint>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

namespace rsa {

// Stateless deleters (function-pointer deleters are not default-constructible
// in libc++, which would delete Key's default ctor).
struct BnFree {
    void operator()(BIGNUM* p) const noexcept { BN_free(p); }
};
struct CtxFree {
    void operator()(BN_CTX* p) const noexcept { BN_CTX_free(p); }
};

// smart pointer because I am smart (lazy too)
using BN_ptr = std::unique_ptr<BIGNUM, BnFree>;
using CTX_ptr = std::unique_ptr<BN_CTX, CtxFree>;

inline BN_ptr make_bn() {
    BIGNUM* b = BN_new();
    if (!b) throw std::runtime_error("BN_new failed");
    return BN_ptr(b);
}
inline BN_ptr bn_from_word(unsigned w) {
    auto b = make_bn();
    if (!BN_set_word(b.get(), w)) throw std::runtime_error("BN_set_word failed");
    return b;
}
inline BN_ptr bn_from_hex(const std::string& h) {
    BIGNUM* raw = nullptr;
    if (!BN_hex2bn(&raw, h.c_str()) || !raw) throw std::runtime_error("bad hex bignum");
    return BN_ptr(raw);
}
inline BN_ptr bn_from_bytes(const uint8_t* p, size_t n) {
    BIGNUM* raw = BN_bin2bn(p, static_cast<int>(n), nullptr);
    if (!raw) throw std::runtime_error("BN_bin2bn failed");
    return BN_ptr(raw);
}
inline std::string bn_to_hex(const BIGNUM* b) {
    char* s = BN_bn2hex(b);
    if (!s) throw std::runtime_error("BN_bn2hex failed");
    std::string out(s);
    OPENSSL_free(s);
    return out;
}

// Straight from the depths of RFC 8017
// I2OSP: fixed-length big-endian serialization (throws if x >= 256^len).
inline std::vector<uint8_t> i2osp(const BIGNUM* x, size_t len) {
    std::vector<uint8_t> out(len);
    if (BN_bn2binpad(x, out.data(), static_cast<int>(len)) != static_cast<int>(len))
        throw std::runtime_error("i2osp: integer too large");
    return out;
}
inline BN_ptr os2ip(const std::vector<uint8_t>& v) {
    return bn_from_bytes(v.data(), v.size());
}
inline BN_ptr mod_exp(const BIGNUM* b, const BIGNUM* e, const BIGNUM* m, BN_CTX* ctx) {
    auto r = make_bn();
    if (!BN_mod_exp(r.get(), b, e, m, ctx)) throw std::runtime_error("BN_mod_exp failed");
    return r;
}

// We will keep following the RFC 8017
// CRT is cool ig
struct Key {
    BN_ptr n, e, d;          // public + private
    BN_ptr p, q;             // primes
    BN_ptr dp, dq, qinv;     // CRT(Chinese Remainder Theorem) params: d mod (p-1), d mod (q-1), q^-1 mod p
    int bits = 0;
};

constexpr unsigned kDefaultE = 65537;  // F4: prime, fast verify (16 squarings + 1 mult) Industry standard

// Generate a `bits`-bit RSA key. p,q are bits/2-bit primes with p,q != 1 mod e.
Key keygen(int bits = 2048, unsigned e_word = kDefaultE) {
    if (bits < 512 || bits % 2) throw std::runtime_error("bits must be an even number >= 512");
    CTX_ptr ctx(BN_CTX_new());
    if (!ctx) throw std::runtime_error("BN_CTX_new failed");
    auto e = bn_from_word(e_word);

    Key k;
    k.bits = bits;
    // Rejection-sample primes with gcd(p-1, e) = 1 (BN_mod_inverse needs it).
    // With prime e=65537 this is just (p-1) % e != 0; probability of a hit is ~2^-16.
    for (;;) {
        // generate p and q
        auto p = make_bn();
        if (!BN_generate_prime_ex(p.get(), bits / 2, 0, nullptr, nullptr, nullptr))
            throw std::runtime_error("prime gen (p) failed");
        if (BN_mod_word(p.get(), e_word) == 1) continue;
        auto q = make_bn();
        if (!BN_generate_prime_ex(q.get(), bits / 2, 0, nullptr, nullptr, nullptr))
            throw std::runtime_error("prime gen (q) failed");
        if (BN_mod_word(q.get(), e_word) == 1) continue;
        if (BN_cmp(p.get(), q.get()) == 0) continue;
        k.p = std::move(p);
        k.q = std::move(q);
        break;
    }
    auto pm1 = make_bn(), qm1 = make_bn(), one = bn_from_word(1);
    if (!BN_sub(pm1.get(), k.p.get(), one.get()) || !BN_sub(qm1.get(), k.q.get(), one.get()))
        throw std::runtime_error("BN_sub failed");
    auto n = make_bn(), phi = make_bn();
    if (!BN_mul(n.get(), k.p.get(), k.q.get(), ctx.get())) throw std::runtime_error("n failed");
    if (!BN_mul(phi.get(), pm1.get(), qm1.get(), ctx.get())) throw std::runtime_error("phi failed");

    BIGNUM* d_raw = BN_mod_inverse(nullptr, e.get(), phi.get(), ctx.get());
    if (!d_raw) throw std::runtime_error("e not invertible mod phi (retry keygen)");
    k.d = BN_ptr(d_raw);
    k.n = std::move(n);
    k.e = std::move(e);
    // CRT params.
    BIGNUM *dp = nullptr, *dq = nullptr, *qi = nullptr;
    dp = BN_new(); dq = BN_new();
    if (!dp || !dq || !BN_mod(dp, k.d.get(), pm1.get(), ctx.get()) ||
        !BN_mod(dq, k.d.get(), qm1.get(), ctx.get()))
        throw std::runtime_error("CRT dp/dq failed");
    qi = BN_mod_inverse(nullptr, k.q.get(), k.p.get(), ctx.get());
    if (!qi) throw std::runtime_error("qinv failed");
    k.dp = BN_ptr(dp);
    k.dq = BN_ptr(dq);
    k.qinv = BN_ptr(qi);
    return k;
}

inline size_t k_bytes(const Key& k) { return (BN_num_bits(k.n.get()) + 7) / 8; }

// raw trapdoor (textbook RSA, NO padding — malleable, demo/KAT only)
inline BN_ptr raw_encrypt(const BN_ptr& m, const Key& k, BN_CTX* ctx) {
    if (BN_cmp(m.get(), k.n.get()) >= 0) throw std::runtime_error("raw_encrypt: m >= n");
    return mod_exp(m.get(), k.e.get(), k.n.get(), ctx);
}
inline BN_ptr raw_decrypt(const BN_ptr& c, const Key& k, BN_CTX* ctx) {
    if (BN_cmp(c.get(), k.n.get()) >= 0) throw std::runtime_error("raw_decrypt: c >= n");
    return mod_exp(c.get(), k.d.get(), k.n.get(), ctx);
}
// CRT decrypt: m1=c^dp mod p, m2=c^dq mod q, h=qinv*(m1-m2) mod p, m=m2+q*h.
// ~4x faster than c^d mod n (cubes of half size); result bit-identical.
inline BN_ptr raw_decrypt_crt(const BN_ptr& c, const Key& k, BN_CTX* ctx) {
    if (BN_cmp(c.get(), k.n.get()) >= 0) throw std::runtime_error("raw_decrypt_crt: c >= n");
    auto m1 = mod_exp(c.get(), k.dp.get(), k.p.get(), ctx);
    auto m2 = mod_exp(c.get(), k.dq.get(), k.q.get(), ctx);
    auto h = make_bn();
    // BN_mod_sub yields (m1-m2) mod p in [0,p) — handles m1 < m2 correctly.
    if (!BN_mod_sub(h.get(), m1.get(), m2.get(), k.p.get(), ctx) ||
        !BN_mod_mul(h.get(), h.get(), k.qinv.get(), k.p.get(), ctx))
        throw std::runtime_error("CRT combine failed");
    auto qh = make_bn(), m = make_bn();
    if (!BN_mul(qh.get(), k.q.get(), h.get(), ctx) || !BN_add(m.get(), m2.get(), qh.get()))
        throw std::runtime_error("CRT final add failed");
    return m;
}

// PKCS#1 v1.5 encryption padding (legacy/compat; NOT CCA-secure, Bleichenbacher Attack)
std::vector<uint8_t> v15_encode(const std::vector<uint8_t>& msg, size_t k) {
    if (msg.size() + 11 > k) throw std::runtime_error("v1.5: message too long");
    size_t ps_len = k - msg.size() - 3;
    std::vector<uint8_t> em(k);
    em[0] = 0x00;
    em[1] = 0x02;
    // PS: nonzero random bytes (zero bytes would terminate the padding early).
    if (RAND_bytes(em.data() + 2, static_cast<int>(ps_len)) != 1)
        throw std::runtime_error("RAND_bytes failed");
    for (size_t i = 0; i < ps_len; ++i)
        while (em[2 + i] == 0x00) {
            if (RAND_bytes(em.data() + 2 + i, 1) != 1) throw std::runtime_error("RAND_bytes failed");
        }
    em[2 + ps_len] = 0x00;
    std::copy(msg.begin(), msg.end(), em.begin() + 3 + ps_len);
    return em;
}

// NOTE: non-constant-time separator scan — fine for a demo, but production
// v1.5 decryption MUST be constant-time (Bleichenbacher '98 oracle).
std::vector<uint8_t> v15_decode(const std::vector<uint8_t>& em) {
    if (em.size() < 11 || em[0] != 0x00 || em[1] != 0x02) throw std::runtime_error("v1.5: bad header");
    size_t i = 2;
    while (i < em.size() && em[i] != 0x00) ++i;
    if (i < 10 || i + 1 >= em.size()) throw std::runtime_error("v1.5: bad padding");
    return std::vector<uint8_t>(em.begin() + i + 1, em.end());
}

// MGF1-SHA256 + OAEP-SHA256 (RFC 8017 §7.1 — the modern choice)
void mgf1_sha256(const uint8_t* seed, size_t seed_len, uint8_t* out, size_t out_len) {
    // One-shot SHA256() (not the deprecated Init/Update/Final trio).
    std::vector<uint8_t> buf;
    buf.reserve(seed_len + 4);
    buf.insert(buf.end(), seed, seed + seed_len);
    uint8_t d[SHA256_DIGEST_LENGTH];
    for (uint32_t ctr = 0; (size_t)ctr * SHA256_DIGEST_LENGTH < out_len; ++ctr) {
        buf.resize(seed_len);
        buf.push_back(ctr >> 24);
        buf.push_back(ctr >> 16);
        buf.push_back(ctr >> 8);
        buf.push_back(ctr);
        SHA256(buf.data(), buf.size(), d);
        size_t off = (size_t)ctr * SHA256_DIGEST_LENGTH;
        size_t take = std::min<size_t>(SHA256_DIGEST_LENGTH, out_len - off);
        std::copy(d, d + take, out + off);
    }
    OPENSSL_cleanse(buf.data(), buf.size());
    OPENSSL_cleanse(d, sizeof d);
}

std::vector<uint8_t> oaep_encode_sha256(const std::vector<uint8_t>& msg, size_t k) {
    constexpr size_t hLen = SHA256_DIGEST_LENGTH;
    if (msg.size() + 2 * hLen + 2 > k) throw std::runtime_error("OAEP: message too long");
    static const uint8_t kEmptyHash[SHA256_DIGEST_LENGTH] = {
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8,
        0x99, 0x6f, 0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c,
        0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55};  // SHA256("")
    size_t db_len = k - hLen - 1;
    std::vector<uint8_t> db(db_len, 0);
    std::copy(kEmptyHash, kEmptyHash + hLen, db.begin());
    db[db_len - msg.size() - 1] = 0x01;
    std::copy(msg.begin(), msg.end(), db.end() - msg.size());

    std::vector<uint8_t> seed(hLen);
    if (RAND_bytes(seed.data(), hLen) != 1) throw std::runtime_error("RAND_bytes failed");
    std::vector<uint8_t> db_mask(db_len), seed_mask(hLen);
    mgf1_sha256(seed.data(), hLen, db_mask.data(), db_len);
    for (size_t i = 0; i < db_len; ++i) db[i] ^= db_mask[i];
    mgf1_sha256(db.data(), db_len, seed_mask.data(), hLen);
    for (size_t i = 0; i < hLen; ++i) seed[i] ^= seed_mask[i];

    std::vector<uint8_t> em;
    em.reserve(k);
    em.push_back(0x00);
    em.insert(em.end(), seed.begin(), seed.end());
    em.insert(em.end(), db.begin(), db.end());
    return em;
}

std::vector<uint8_t> oaep_decode_sha256(const std::vector<uint8_t>& em, size_t k) {
    constexpr size_t hLen = SHA256_DIGEST_LENGTH;
    if (em.size() != k || k < 2 * hLen + 2 || em[0] != 0x00)
        throw std::runtime_error("OAEP: bad length/header");
    // NOTE: early-return checks below make this non-constant-time (demo only);
    // production OAEP decode must run in constant time (Manger '01 oracle).
    const uint8_t* masked_seed = em.data() + 1;
    const uint8_t* masked_db = em.data() + 1 + hLen;
    size_t db_len = k - hLen - 1;
    std::vector<uint8_t> seed_mask(hLen), seed(hLen, 0), db_mask(db_len), db(db_len);
    mgf1_sha256(masked_db, db_len, seed_mask.data(), hLen);
    for (size_t i = 0; i < hLen; ++i) seed[i] = masked_seed[i] ^ seed_mask[i];
    mgf1_sha256(seed.data(), hLen, db_mask.data(), db_len);
    for (size_t i = 0; i < db_len; ++i) db[i] = masked_db[i] ^ db_mask[i];

    static const uint8_t kEmptyHash[SHA256_DIGEST_LENGTH] = {
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8,
        0x99, 0x6f, 0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c,
        0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55};
    if (std::equal(db.begin(), db.begin() + hLen, kEmptyHash)) {
        /* lHash OK */
    } else {
        throw std::runtime_error("OAEP: lHash mismatch");
    }
    size_t i = hLen;
    while (i < db_len && db[i] == 0x00) ++i;
    if (i >= db_len || db[i] != 0x01) throw std::runtime_error("OAEP: separator not found");
    return std::vector<uint8_t>(db.begin() + i + 1, db.end());
}

// RSASSA-PKCS1-v1_5-SHA256 signatures
constexpr uint8_t kSha256DigestInfoPrefix[19] = {0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60,
                                                0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
                                                0x01, 0x05, 0x00, 0x04, 0x20};

bool secure_equal(const std::vector<uint8_t>& a, const std::vector<uint8_t>& b) {
    if (a.size() != b.size()) return false;
    volatile uint8_t d = 0;
    for (size_t i = 0; i < a.size(); ++i) d |= (a[i] ^ b[i]);
    return d == 0;
}

std::vector<uint8_t> sign_v15_sha256(const std::vector<uint8_t>& msg, const Key& k, BN_CTX* ctx) {
    size_t kk = k_bytes(k);
    uint8_t h[SHA256_DIGEST_LENGTH];
    SHA256(msg.data(), msg.size(), h);
    std::vector<uint8_t> t(kSha256DigestInfoPrefix, kSha256DigestInfoPrefix + 19);
    t.insert(t.end(), h, h + SHA256_DIGEST_LENGTH);  // T = DigestInfo (51 bytes)
    if (kk < t.size() + 11) throw std::runtime_error("sign: key too small");
    std::vector<uint8_t> em(kk, 0);
    em[0] = 0x00;
    em[1] = 0x01;
    size_t ps_len = kk - t.size() - 3;
    std::fill(em.begin() + 2, em.begin() + 2 + ps_len, 0xFF);
    em[2 + ps_len] = 0x00;
    std::copy(t.begin(), t.end(), em.begin() + 3 + ps_len);
    return i2osp(raw_decrypt(os2ip(em), k, ctx).get(), kk);  // s = EM^d mod n
}

bool verify_v15_sha256(const std::vector<uint8_t>& msg, const std::vector<uint8_t>& sig,
                       const Key& k, BN_CTX* ctx) {
    size_t kk = k_bytes(k);
    if (sig.size() != kk) return false;
    BN_ptr s, em_bn;
    try {
        s = os2ip(sig);
        if (BN_cmp(s.get(), k.n.get()) >= 0) return false;
        em_bn = raw_encrypt(s, k, ctx);  // EM' = s^e mod n
    } catch (...) {
        return false;
    }
    std::vector<uint8_t> em;
    try {
        em = i2osp(em_bn.get(), kk);
    } catch (...) {
        return false;
    }
    // Rebuild expected EM and compare in constant time.
    uint8_t h[SHA256_DIGEST_LENGTH];
    SHA256(msg.data(), msg.size(), h);
    std::vector<uint8_t> t(kSha256DigestInfoPrefix, kSha256DigestInfoPrefix + 19);
    t.insert(t.end(), h, h + SHA256_DIGEST_LENGTH);
    if (kk < t.size() + 11) return false;
    std::vector<uint8_t> want(kk, 0);
    want[0] = 0x00;
    want[1] = 0x01;
    size_t ps_len = kk - t.size() - 3;
    std::fill(want.begin() + 2, want.begin() + 2 + ps_len, 0xFF);
    want[2 + ps_len] = 0x00;
    std::copy(t.begin(), t.end(), want.begin() + 3 + ps_len);
    return secure_equal(em, want);
}

}  // namespace rsa

// hex helpers
static std::vector<uint8_t> from_hex(const std::string& h) {
    if (h.size() % 2) throw std::runtime_error("odd-length hex");
    auto nib = [](char c) -> int {
        if (c >= '0' && c <= '9') return c - '0';
        if (c >= 'a' && c <= 'f') return c - 'a' + 10;
        if (c >= 'A' && c <= 'F') return c - 'A' + 10;
        throw std::runtime_error("bad hex char");
    };
    std::vector<uint8_t> o;
    o.reserve(h.size() / 2);
    for (size_t i = 0; i < h.size(); i += 2)
        o.push_back((nib(h[i]) << 4) | nib(h[i + 1]));
    return o;
}
static std::string to_hex(const std::vector<uint8_t>& v) {
    static const char* d = "0123456789abcdef";
    std::string o;
    o.reserve(v.size() * 2);
    for (auto b : v) {
        o += d[b >> 4];
        o += d[b & 15];
    }
    return o;
}

// ---- self-tests ----
static int run_selftests() {
    using namespace rsa;
    int fails = 0;
    auto check = [&](const char* n, bool ok, const std::string& extra = "") {
        std::cout << (ok ? "[PASS] " : "[FAIL] ") << n;
        if (!extra.empty()) std::cout << " (" << extra << ")";
        std::cout << "\n";
        if (!ok) ++fails;
    };

    // 1. Textbook KAT (Stallings): p=61, q=53, n=3233, e=17, d=2753.
    //    m=65 -> c=2790. Exercises raw_encrypt/raw_decrypt with known answers.
    {
        CTX_ptr ctx(BN_CTX_new());
        Key t;
        t.n = bn_from_word(3233);
        t.e = bn_from_word(17);
        t.d = bn_from_word(2753);
        t.p = bn_from_word(61);
        t.q = bn_from_word(53);
        t.dp = bn_from_word(53 % 60);  // d mod (p-1) = 2753 mod 60 = 53
        t.dq = bn_from_word(2753 % 52);  // d mod (q-1) = 2753 mod 52 = 49
        t.qinv = bn_from_word(38);       // q^-1 mod p = 53^-1 mod 61 = 38
        auto m = bn_from_word(65);
        auto c = raw_encrypt(m, t, ctx.get());
        check("KAT encrypt (65^17 mod 3233 == 2790)", BN_get_word(c.get()) == 2790);
        auto m2 = raw_decrypt(c, t, ctx.get());
        check("KAT decrypt (2790^2753 mod 3233 == 65)", BN_get_word(m2.get()) == 65);
        auto m3 = raw_decrypt_crt(c, t, ctx.get());
        check("KAT CRT decrypt == 65", BN_get_word(m3.get()) == 65);
    }

    // 2. Fresh 2048-bit keygen + structural checks.
    auto t0 = std::chrono::steady_clock::now();
    Key k = keygen(2048);
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(
                  std::chrono::steady_clock::now() - t0)
                  .count();
    check("keygen 2048-bit n", BN_num_bits(k.n.get()) == 2048,
          "took " + std::to_string(ms) + "ms");
    {
        CTX_ptr ctx(BN_CTX_new());
        // e*d == 1 mod lcm(p-1,q-1): verify via ed mod (p-1) == 1 and mod (q-1) == 1.
        auto pm1 = make_bn(), qm1 = make_bn(), one = bn_from_word(1), ed = make_bn();
        BN_sub(pm1.get(), k.p.get(), one.get());
        BN_sub(qm1.get(), k.q.get(), one.get());
        BN_mul(ed.get(), k.e.get(), k.d.get(), ctx.get());
        auto r1 = make_bn(), r2 = make_bn();
        BN_mod(r1.get(), ed.get(), pm1.get(), ctx.get());
        BN_mod(r2.get(), ed.get(), qm1.get(), ctx.get());
        check("e*d == 1 mod (p-1) and mod (q-1)", BN_cmp(r1.get(), one.get()) == 0 &&
                                                     BN_cmp(r2.get(), one.get()) == 0);
        check("p != q", BN_cmp(k.p.get(), k.q.get()) != 0);
        check("n == p*q", [&] {
            auto n2 = make_bn();
            BN_mul(n2.get(), k.p.get(), k.q.get(), ctx.get());
            return BN_cmp(n2.get(), k.n.get()) == 0;
        }());
    }

    // 3. Round-trips on the fresh key.
    {
        CTX_ptr ctx(BN_CTX_new());
        size_t kk = k_bytes(k);
        check("k == 256 bytes", kk == 256);

        // raw round-trip
        std::vector<uint8_t> mraw = {0x48, 0x65, 0x6c, 0x6c, 0x6f};  // "Hello" < n
        auto m = os2ip(mraw);
        auto c = raw_encrypt(m, k, ctx.get());
        auto m_classic = raw_decrypt(c, k, ctx.get());
        auto m_crt = raw_decrypt_crt(c, k, ctx.get());
        check("raw round-trip", BN_cmp(m_classic.get(), m.get()) == 0);
        check("CRT == classical", BN_cmp(m_crt.get(), m_classic.get()) == 0);

        // v1.5 round-trip
        std::vector<uint8_t> msg = {'H', 'e', 'l', 'l', 'o', ',', ' ', 'R', 'S', 'A'};
        auto em = v15_encode(msg, kk);
        auto c2 = raw_encrypt(os2ip(em), k, ctx.get());
        auto em2 = i2osp(raw_decrypt_crt(c2, k, ctx.get()).get(), kk);
        check("PKCS#1 v1.5 round-trip", v15_decode(em2) == msg);
        // v1.5 tamper: corrupt padding header -> decode must throw
        {
            bool threw = false;
            em2[1] = 0x03;
            try {
                v15_decode(em2);
            } catch (...) {
                threw = true;
            }
            check("v1.5 tamper rejection", threw);
        }

        // OAEP-SHA256 round-trip
        auto oem = oaep_encode_sha256(msg, kk);
        auto oc = raw_encrypt(os2ip(oem), k, ctx.get());
        auto oem2 = i2osp(raw_decrypt_crt(oc, k, ctx.get()).get(), kk);
        check("OAEP-SHA256 round-trip", oaep_decode_sha256(oem2, kk) == msg);
        // OAEP tamper
        {
            bool threw = false;
            oem2[kk - 1] ^= 0x01;
            try {
                oaep_decode_sha256(oem2, kk);
            } catch (...) {
                threw = true;
            }
            check("OAEP tamper rejection", threw);
        }

        // sign / verify
        std::vector<uint8_t> doc = {'s', 'i', 'g', 'n', ' ', 'm', 'e'};
        auto sig = sign_v15_sha256(doc, k, ctx.get());
        check("sign length == k", sig.size() == kk);
        check("verify valid sig", verify_v15_sha256(doc, sig, k, ctx.get()));
        std::vector<uint8_t> bad_doc = {'s', 'i', 'g', 'n', ' ', 'X', 'e'};
        check("verify rejects wrong message", !verify_v15_sha256(bad_doc, sig, k, ctx.get()));
        auto bad_sig = sig;
        bad_sig[kk / 2] ^= 0x01;
        check("verify rejects flipped sig", !verify_v15_sha256(doc, bad_sig, k, ctx.get()));
    }

    std::cout << (fails ? "\nSELF-TEST FAILED\n" : "\nALL SELF-TESTS PASSED\n");
    return fails ? 1 : 0;
}

int main(int argc, char** argv) {
    using namespace rsa;
    std::string mode = argc > 1 ? argv[1] : "selftest";
    try {
        if (mode == "selftest" || mode == "test") return run_selftests();
        if (mode == "keygen") {
            int bits = argc > 2 ? std::stoi(argv[2]) : 2048;
            Key k = keygen(bits);
            std::cout << "bits=" << bits << "\n";
            std::cout << "n=" << bn_to_hex(k.n.get()) << "\n";
            std::cout << "e=" << bn_to_hex(k.e.get()) << "\n";
            std::cout << "d=" << bn_to_hex(k.d.get()) << "\n";
            std::cout << "p=" << bn_to_hex(k.p.get()) << "\n";
            std::cout << "q=" << bn_to_hex(k.q.get()) << "\n";
            std::cout << "dp=" << bn_to_hex(k.dp.get()) << "\n";
            std::cout << "dq=" << bn_to_hex(k.dq.get()) << "\n";
            std::cout << "qinv=" << bn_to_hex(k.qinv.get()) << "\n";
            return 0;
        }
        if (mode == "raw-enc" && argc == 5) {
            CTX_ptr ctx(BN_CTX_new());
            Key k;
            k.n = bn_from_hex(argv[2]);
            k.e = bn_from_hex(argv[3]);
            auto m = bn_from_hex(argv[4]);
            std::cout << bn_to_hex(raw_encrypt(m, k, ctx.get()).get()) << "\n";
            return 0;
        }
        if (mode == "raw-dec" && argc == 5) {
            CTX_ptr ctx(BN_CTX_new());
            Key k;
            k.n = bn_from_hex(argv[2]);
            k.d = bn_from_hex(argv[3]);
            std::vector<uint8_t> cb = from_hex(argv[4]);
            auto c = bn_from_bytes(cb.data(), cb.size());
            auto m = raw_decrypt(c, k, ctx.get());
            // minimal-length output (no leading zeros)
            int nbytes = (BN_num_bits(m.get()) + 7) / 8;
            std::vector<uint8_t> out(nbytes ? nbytes : 1);
            BN_bn2bin(m.get(), out.data());
            std::cout << to_hex(out) << "\n";
            return 0;
        }
        std::cerr << "usage:\n  " << argv[0] << " [selftest]\n  " << argv[0]
                  << " keygen [bits]\n  " << argv[0] << " raw-enc <n_hex> <e_hex> <m_hex>\n  "
                  << argv[0] << " raw-dec <n_hex> <d_hex> <c_hex>\n";
        return 2;
    } catch (const std::exception& ex) {
        std::cerr << "error: " << ex.what() << "\n";
        return 1;
    }
}
