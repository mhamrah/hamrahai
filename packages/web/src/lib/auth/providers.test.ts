import { describe, it, expect, vi, beforeEach } from 'vitest';
import { verifyGoogleToken, verifyAppleToken } from './providers';
import { mockFetchResponse } from '../../test/setup';

// Mock jose library
vi.mock('jose', () => ({
  jwtVerify: vi.fn(),
  importJWK: vi.fn(),
}));

describe('OAuth Provider Token Verification', () => {
  const mockEvent = {
    platform: {
      env: {
        GOOGLE_CLIENT_ID: 'your-google-client-id',
        APPLE_CLIENT_ID: 'your-apple-client-id'
      }
    }
  };

  beforeEach(() => {
    vi.clearAllMocks();
    global.fetch = vi.fn();
  });

  describe('verifyGoogleToken', () => {
    it('should verify valid Google ID token', async () => {
      // Mock Google JWKS response
      const mockJWKS = {
        keys: [
          {
            kid: 'test-key-id',
            kty: 'RSA',
            n: 'test-modulus',
            e: 'AQAB',
          },
        ],
      };

      global.fetch = mockFetchResponse(mockJWKS);

      // Mock JWT verification
      const { jwtVerify, importJWK } = await import('jose');
      vi.mocked(importJWK).mockResolvedValue({} as any);
      vi.mocked(jwtVerify).mockResolvedValue({
        payload: {
          sub: 'google-user-id-123',
          email: 'test@gmail.com',
          name: 'Test User',
          picture: 'https://example.com/avatar.jpg',
        },
      } as any);

      // Mock token with test key ID
      const mockToken = 'header.payload.signature';
      vi.spyOn(global, 'atob').mockReturnValue(JSON.stringify({ kid: 'test-key-id' }));

      const result = await verifyGoogleToken(mockToken, mockEvent as any);

      expect(result).toEqual({
        email: 'test@gmail.com',
        name: 'Test User',
        picture: 'https://example.com/avatar.jpg',
        providerId: 'google-user-id-123',
      });

      expect(fetch).toHaveBeenCalledWith('https://www.googleapis.com/oauth2/v3/certs');
      expect(jwtVerify).toHaveBeenCalledWith(
        mockToken,
        {},
        {
          issuer: ['https://accounts.google.com', 'accounts.google.com'],
          audience: [
            'your-google-client-id',
            '107139115848-jvf449cojr174ocan4vpanddh8i48oko.apps.googleusercontent.com',
          ],
        }
      );
    });

    it('should reject token with invalid signature', async () => {
      const mockJWKS = {
        keys: [{ kid: 'test-key-id', kty: 'RSA' }],
      };

      global.fetch = mockFetchResponse(mockJWKS);

      const { jwtVerify, importJWK } = await import('jose');
      vi.mocked(importJWK).mockResolvedValue({} as any);
      vi.mocked(jwtVerify).mockRejectedValue(new Error('Invalid signature'));

      vi.spyOn(global, 'atob').mockReturnValue(JSON.stringify({ kid: 'test-key-id' }));

      await expect(verifyGoogleToken('invalid.token.signature', mockEvent as any)).rejects.toThrow('Invalid Google token');
    });

    it('should reject token without email', async () => {
      const mockJWKS = {
        keys: [{ kid: 'test-key-id', kty: 'RSA' }],
      };

      global.fetch = mockFetchResponse(mockJWKS);

      const { jwtVerify, importJWK } = await import('jose');
      vi.mocked(importJWK).mockResolvedValue({} as any);
      vi.mocked(jwtVerify).mockResolvedValue({
        payload: {
          sub: 'google-user-id-123',
          // Missing email
          name: 'Test User',
        },
      } as any);

      vi.spyOn(global, 'atob').mockReturnValue(JSON.stringify({ kid: 'test-key-id' }));

      await expect(verifyGoogleToken('token.without.email', mockEvent as any)).rejects.toThrow('Invalid Google token');
    });

    it('should handle missing key ID in JWKS', async () => {
      const mockJWKS = {
        keys: [{ kid: 'different-key-id', kty: 'RSA' }],
      };

      global.fetch = mockFetchResponse(mockJWKS);

      vi.spyOn(global, 'atob').mockReturnValue(JSON.stringify({ kid: 'missing-key-id' }));

      await expect(verifyGoogleToken('token.with.missing.key', mockEvent as any)).rejects.toThrow('Invalid Google token');
    });
  });

  describe('verifyAppleToken', () => {
    it('should verify valid Apple ID token', async () => {
      // Mock Apple JWKS response
      const mockJWKS = {
        keys: [
          {
            kid: 'apple-key-id',
            kty: 'RSA',
            n: 'apple-modulus',
            e: 'AQAB',
          },
        ],
      };

      global.fetch = mockFetchResponse(mockJWKS);

      const { jwtVerify, importJWK } = await import('jose');
      vi.mocked(importJWK).mockResolvedValue({} as any);
      vi.mocked(jwtVerify).mockResolvedValue({
        payload: {
          sub: 'apple-user-id-456',
          email: 'test@privaterelay.appleid.com',
          // Apple doesn't always provide name/picture
        },
      } as any);

      vi.spyOn(global, 'atob').mockReturnValue(JSON.stringify({ kid: 'apple-key-id' }));

      const result = await verifyAppleToken('apple.id.token', mockEvent as any);

      expect(result).toEqual({
        email: 'test@privaterelay.appleid.com',
        name: undefined,
        picture: undefined,
        providerId: 'apple-user-id-456',
      });

      expect(fetch).toHaveBeenCalledWith('https://appleid.apple.com/auth/keys');
      expect(jwtVerify).toHaveBeenCalledWith(
        'apple.id.token',
        {},
        {
          issuer: 'https://appleid.apple.com',
          audience: ['your-apple-client-id', 'app.hamrah.ios'],
        }
      );
    });

    it('should reject invalid Apple token', async () => {
      const mockJWKS = {
        keys: [{ kid: 'apple-key-id', kty: 'RSA' }],
      };

      global.fetch = mockFetchResponse(mockJWKS);

      const { jwtVerify, importJWK } = await import('jose');
      vi.mocked(importJWK).mockResolvedValue({} as any);
      vi.mocked(jwtVerify).mockRejectedValue(new Error('Token expired'));

      vi.spyOn(global, 'atob').mockReturnValue(JSON.stringify({ kid: 'apple-key-id' }));

      await expect(verifyAppleToken('expired.apple.token', mockEvent as any)).rejects.toThrow('Invalid Apple token');
    });

    it('should handle JWKS fetch failure', async () => {
      global.fetch = vi.fn().mockRejectedValue(new Error('Network error'));

      await expect(verifyGoogleToken('any.token.here', mockEvent as any)).rejects.toThrow('Invalid Google token');
      await expect(verifyAppleToken('any.apple.token', mockEvent as any)).rejects.toThrow('Invalid Apple token');
    });
  });

  describe('Error Handling', () => {
    it('should handle malformed JWT header', async () => {
      vi.spyOn(global, 'atob').mockImplementation(() => {
        throw new Error('Invalid base64');
      });

      await expect(verifyGoogleToken('malformed.token', mockEvent as any)).rejects.toThrow('Invalid Google token');
      await expect(verifyAppleToken('malformed.token', mockEvent as any)).rejects.toThrow('Invalid Apple token');
    });

    it('should handle invalid JSON in JWT header', async () => {
      vi.spyOn(global, 'atob').mockReturnValue('invalid-json');

      await expect(verifyGoogleToken('invalid.header.token', mockEvent as any)).rejects.toThrow('Invalid Google token');
      await expect(verifyAppleToken('invalid.header.token', mockEvent as any)).rejects.toThrow('Invalid Apple token');
    });
  });
});