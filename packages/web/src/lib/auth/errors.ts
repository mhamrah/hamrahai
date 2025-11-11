/**
 * Standardized error handling for authentication and authorization
 */

export interface AuthError {
  code: string;
  message: string;
  httpStatus: number;
  details?: any;
}

export interface OAuthError {
  error: string;
  error_description: string;
  error_uri?: string;
}

/**
 * Standard OAuth 2.0 error codes
 */
export const OAUTH_ERRORS = {
  INVALID_REQUEST: 'invalid_request',
  INVALID_CLIENT: 'invalid_client', 
  INVALID_GRANT: 'invalid_grant',
  UNAUTHORIZED_CLIENT: 'unauthorized_client',
  UNSUPPORTED_GRANT_TYPE: 'unsupported_grant_type',
  INVALID_SCOPE: 'invalid_scope',
  SERVER_ERROR: 'server_error',
  TEMPORARILY_UNAVAILABLE: 'temporarily_unavailable',
  UNSUPPORTED_RESPONSE_TYPE: 'unsupported_response_type',
  ACCESS_DENIED: 'access_denied',
} as const;

/**
 * Standard HTTP status codes for auth errors
 */
export const AUTH_HTTP_STATUS = {
  BAD_REQUEST: 400,
  UNAUTHORIZED: 401,
  FORBIDDEN: 403,
  NOT_FOUND: 404,
  TOO_MANY_REQUESTS: 429,
  INTERNAL_SERVER_ERROR: 500,
} as const;

/**
 * Create standardized OAuth error response
 */
export function createOAuthError(
  error: string,
  description: string,
  status: number = AUTH_HTTP_STATUS.BAD_REQUEST
): { status: number; body: OAuthError } {
  return {
    status,
    body: {
      error,
      error_description: description,
    },
  };
}

/**
 * Create standardized auth error
 */
export function createAuthError(
  code: string,
  message: string,
  httpStatus: number = AUTH_HTTP_STATUS.INTERNAL_SERVER_ERROR,
  details?: any
): AuthError {
  return {
    code,
    message,
    httpStatus,
    details,
  };
}

/**
 * Common OAuth error creators
 */
export const oauthErrors = {
  invalidRequest: (description: string) => 
    createOAuthError(OAUTH_ERRORS.INVALID_REQUEST, description, AUTH_HTTP_STATUS.BAD_REQUEST),
    
  invalidClient: (description: string) => 
    createOAuthError(OAUTH_ERRORS.INVALID_CLIENT, description, AUTH_HTTP_STATUS.UNAUTHORIZED),
    
  invalidGrant: (description: string) => 
    createOAuthError(OAUTH_ERRORS.INVALID_GRANT, description, AUTH_HTTP_STATUS.BAD_REQUEST),
    
  unauthorizedClient: (description: string) => 
    createOAuthError(OAUTH_ERRORS.UNAUTHORIZED_CLIENT, description, AUTH_HTTP_STATUS.UNAUTHORIZED),
    
  unsupportedGrantType: (description: string) => 
    createOAuthError(OAUTH_ERRORS.UNSUPPORTED_GRANT_TYPE, description, AUTH_HTTP_STATUS.BAD_REQUEST),
    
  invalidScope: (description: string) => 
    createOAuthError(OAUTH_ERRORS.INVALID_SCOPE, description, AUTH_HTTP_STATUS.BAD_REQUEST),
    
  serverError: (description: string = 'Internal server error') => 
    createOAuthError(OAUTH_ERRORS.SERVER_ERROR, description, AUTH_HTTP_STATUS.INTERNAL_SERVER_ERROR),
    
  temporarilyUnavailable: (description: string) => 
    createOAuthError(OAUTH_ERRORS.TEMPORARILY_UNAVAILABLE, description, AUTH_HTTP_STATUS.INTERNAL_SERVER_ERROR),
    
  unsupportedResponseType: (description: string) => 
    createOAuthError(OAUTH_ERRORS.UNSUPPORTED_RESPONSE_TYPE, description, AUTH_HTTP_STATUS.BAD_REQUEST),
    
  accessDenied: (description: string) => 
    createOAuthError(OAUTH_ERRORS.ACCESS_DENIED, description, AUTH_HTTP_STATUS.FORBIDDEN),
};

/**
 * Common auth error creators
 */
export const authErrors = {
  unauthorized: (message: string = 'Authentication required') =>
    createAuthError('AUTH_UNAUTHORIZED', message, AUTH_HTTP_STATUS.UNAUTHORIZED),
    
  forbidden: (message: string = 'Access forbidden') =>
    createAuthError('AUTH_FORBIDDEN', message, AUTH_HTTP_STATUS.FORBIDDEN),
    
  tokenExpired: (message: string = 'Token has expired') =>
    createAuthError('AUTH_TOKEN_EXPIRED', message, AUTH_HTTP_STATUS.UNAUTHORIZED),
    
  invalidToken: (message: string = 'Invalid token') =>
    createAuthError('AUTH_INVALID_TOKEN', message, AUTH_HTTP_STATUS.UNAUTHORIZED),
    
  rateLimited: (message: string = 'Rate limit exceeded') =>
    createAuthError('AUTH_RATE_LIMITED', message, AUTH_HTTP_STATUS.TOO_MANY_REQUESTS),
    
  userNotFound: (message: string = 'User not found') =>
    createAuthError('AUTH_USER_NOT_FOUND', message, AUTH_HTTP_STATUS.NOT_FOUND),
    
  internalError: (message: string = 'Internal server error', details?: any) =>
    createAuthError('AUTH_INTERNAL_ERROR', message, AUTH_HTTP_STATUS.INTERNAL_SERVER_ERROR, details),
};

/**
 * Result wrapper for operations that may fail
 */
export type AuthResult<T> = 
  | { success: true; data: T }
  | { success: false; error: AuthError };

/**
 * Create successful result
 */
export function success<T>(data: T): AuthResult<T> {
  return { success: true, data };
}

/**
 * Create error result
 */
export function failure<T>(error: AuthError): AuthResult<T> {
  return { success: false, error };
}

/**
 * Wrap async function with error handling
 */
export async function withErrorHandling<T>(
  fn: () => Promise<T>,
  fallbackError?: AuthError
): Promise<AuthResult<T>> {
  try {
    const result = await fn();
    return success(result);
  } catch (error) {
    console.error('Auth operation failed:', error);
    
    if (error instanceof Error) {
      return failure(
        fallbackError || authErrors.internalError(error.message, { stack: error.stack })
      );
    }
    
    return failure(fallbackError || authErrors.internalError('Unknown error', { error }));
  }
}