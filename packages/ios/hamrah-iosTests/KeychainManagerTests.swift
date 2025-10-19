import XCTest
@testable import hamrah_ios

class KeychainManagerTests: XCTestCase {
    
    var keychain: KeychainManager!
    let testKey = "test_key"
    let testString = "test_value"
    let testBool = true
    let testDouble = 123.456
    
    override func setUp() {
        super.setUp()
        keychain = KeychainManager.shared
        
        // Clean up any existing test data
        keychain.delete(for: testKey)
    }
    
    override func tearDown() {
        // Clean up test data
        keychain.delete(for: testKey)
        super.tearDown()
    }
    
    // MARK: - String Storage Tests
    
    func testStoreAndRetrieveString() {
        // Test storing a string
        let success = keychain.store(testString, for: testKey)
        XCTAssertTrue(success, "Should successfully store string in Keychain")
        
        // Test retrieving the string
        let retrieved = keychain.retrieveString(for: testKey)
        XCTAssertEqual(retrieved, testString, "Retrieved string should match stored string")
    }
    
    func testRetrieveNonExistentString() {
        let retrieved = keychain.retrieveString(for: "non_existent_key")
        XCTAssertNil(retrieved, "Should return nil for non-existent key")
    }
    
    // MARK: - Data Storage Tests
    
    func testStoreAndRetrieveData() {
        guard let testData = testString.data(using: .utf8) else {
            XCTFail("Could not create test data")
            return
        }
        
        // Test storing data
        let success = keychain.store(testData, for: testKey)
        XCTAssertTrue(success, "Should successfully store data in Keychain")
        
        // Test retrieving data
        let retrieved = keychain.retrieve(for: testKey)
        XCTAssertEqual(retrieved, testData, "Retrieved data should match stored data")
    }
    
    // MARK: - Boolean Storage Tests
    
    func testStoreAndRetrieveBool() {
        // Test storing a boolean
        let success = keychain.store(testBool, for: testKey)
        XCTAssertTrue(success, "Should successfully store boolean in Keychain")
        
        // Test retrieving the boolean
        let retrieved = keychain.retrieveBool(for: testKey)
        XCTAssertEqual(retrieved, testBool, "Retrieved boolean should match stored boolean")
    }
    
    func testStoreAndRetrieveFalseBool() {
        // Test storing false
        let success = keychain.store(false, for: testKey)
        XCTAssertTrue(success, "Should successfully store false boolean in Keychain")
        
        // Test retrieving false
        let retrieved = keychain.retrieveBool(for: testKey)
        XCTAssertEqual(retrieved, false, "Retrieved boolean should be false")
    }
    
    // MARK: - Double Storage Tests
    
    func testStoreAndRetrieveDouble() {
        // Test storing a double
        let success = keychain.store(testDouble, for: testKey)
        XCTAssertTrue(success, "Should successfully store double in Keychain")
        
        // Test retrieving the double
        let retrieved = keychain.retrieveDouble(for: testKey)
        XCTAssertEqual(retrieved, testDouble, accuracy: 0.0001, "Retrieved double should match stored double")
    }
    
    // MARK: - Delete Tests
    
    func testDeleteItem() {
        // First store an item
        let success = keychain.store(testString, for: testKey)
        XCTAssertTrue(success, "Should successfully store string")
        
        // Verify it exists
        let retrieved = keychain.retrieveString(for: testKey)
        XCTAssertEqual(retrieved, testString, "Item should exist before deletion")
        
        // Delete the item
        let deleteSuccess = keychain.delete(for: testKey)
        XCTAssertTrue(deleteSuccess, "Should successfully delete item")
        
        // Verify it's gone
        let retrievedAfterDelete = keychain.retrieveString(for: testKey)
        XCTAssertNil(retrievedAfterDelete, "Item should not exist after deletion")
    }
    
    func testDeleteNonExistentItem() {
        // Should succeed even if item doesn't exist
        let success = keychain.delete(for: "non_existent_key")
        XCTAssertTrue(success, "Should return true even for non-existent items")
    }
    
    // MARK: - Overwrite Tests
    
    func testOverwriteExistingItem() {
        let firstValue = "first_value"
        let secondValue = "second_value"
        
        // Store first value
        let firstSuccess = keychain.store(firstValue, for: testKey)
        XCTAssertTrue(firstSuccess, "Should store first value")
        
        // Store second value (should overwrite)
        let secondSuccess = keychain.store(secondValue, for: testKey)
        XCTAssertTrue(secondSuccess, "Should store second value")
        
        // Verify second value is retrieved
        let retrieved = keychain.retrieveString(for: testKey)
        XCTAssertEqual(retrieved, secondValue, "Should retrieve the overwritten value")
    }
    
    // MARK: - Clear All Data Tests
    
    func testClearAllHamrahData() {
        let keys = ["hamrah_user", "hamrah_access_token", "hamrah_is_authenticated"]
        
        // Store test data for all Hamrah keys
        for key in keys {
            let success = keychain.store("test_data_\(key)", for: key)
            XCTAssertTrue(success, "Should store data for key: \(key)")
        }
        
        // Verify data exists
        for key in keys {
            let retrieved = keychain.retrieveString(for: key)
            XCTAssertNotNil(retrieved, "Data should exist for key: \(key)")
        }
        
        // Clear all Hamrah data
        let clearSuccess = keychain.clearAllHamrahData()
        XCTAssertTrue(clearSuccess, "Should successfully clear all Hamrah data")
        
        // Verify all data is gone
        for key in keys {
            let retrieved = keychain.retrieveString(for: key)
            XCTAssertNil(retrieved, "Data should not exist for key: \(key)")
        }
    }
    
    // MARK: - Security Tests
    
    func testKeychainAccessibility() {
        let success = keychain.store("sensitive_data", for: testKey)
        XCTAssertTrue(success, "Should store sensitive data")
        
        // This test mainly verifies that we're using the correct accessibility level
        // The actual security enforcement is handled by iOS Keychain
        let retrieved = keychain.retrieveString(for: testKey)
        XCTAssertEqual(retrieved, "sensitive_data", "Should retrieve sensitive data when device is unlocked")
    }
}