-- Sprint 9 Fix: Reset admin password hash to known 'admin123'
-- Issue #110: Test environment login fails
-- The original seed hash was generated with an unknown password.
-- This migration ensures the admin user has the password 'admin123' for testing.

UPDATE users
SET password_hash = '$argon2id$v=19$m=19456,t=2,p=1$IZn2RlHZnnlGUVpJNnDYNA$k9mtZYMO/LA3j6Jh0ejUlcEs15OB74RFg2peA7qQvL8'
WHERE username = 'admin';
