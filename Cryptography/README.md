What OpenSSL does: BIGNUM allocation/arithmetic, random prime generation, modular inversion, and SHA-256.

What I implement: RSA key construction logic, RSA exponentiation flow, CRT recombination, I2OSP/OS2IP, PKCS#1 v1.5 encoding/decoding, OAEP/MGF1, DigestInfo construction, signature verification, and the vulnerable oracle/timing harness.

What I deliberately don't use: OpenSSL's high-level RSA encryption/signature APIs.