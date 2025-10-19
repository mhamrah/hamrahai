import XCTest

@testable import hamrah_ios

final class CanonicalizerTests: XCTestCase {

    func testCanonicalization_basic() {
        let url = URL(
            string: "http://Example.com:80/Some/Path/?utm_source=google&gclid=abc&foo=bar#fragment"
        )!
        let canon = Canonicalizer.canonicalize(url: url)
        XCTAssertEqual(canon, "https://example.com/Some/Path?foo=bar")
    }

    func testCanonicalization_duplicateSlashes() {
        let url = URL(string: "https://example.com//foo///bar//baz/")!
        let canon = Canonicalizer.canonicalize(url: url)
        XCTAssertEqual(canon, "https://example.com/foo/bar/baz")
    }

    func testCanonicalization_stripSession() {
        let url = URL(string: "https://example.com/page?sid=123&PHPSESSID=456&foo=bar")!
        let canon = Canonicalizer.canonicalize(url: url)
        XCTAssertEqual(canon, "https://example.com/page?foo=bar")
    }

    func testCanonicalization_trailingSlashRoot() {
        let url = URL(string: "https://example.com/")!
        let canon = Canonicalizer.canonicalize(url: url)
        XCTAssertEqual(canon, "https://example.com/")
    }

    func testCanonicalization_trailingSlashNonRoot() {
        let url = URL(string: "https://example.com/foo/")!
        let canon = Canonicalizer.canonicalize(url: url)
        XCTAssertEqual(canon, "https://example.com/foo")
    }

    func testCanonicalization_removeFragment() {
        let url = URL(string: "https://example.com/foo#section2")!
        let canon = Canonicalizer.canonicalize(url: url)
        XCTAssertEqual(canon, "https://example.com/foo")
    }

    func testCanonicalization_removeTrackingParams() {
        let url = URL(
            string: "https://example.com/foo?utm_source=abc&utm_medium=def&fbclid=xyz&bar=baz"
        )!
        let canon = Canonicalizer.canonicalize(url: url)
        XCTAssertEqual(canon, "https://example.com/foo?bar=baz")
    }
}
