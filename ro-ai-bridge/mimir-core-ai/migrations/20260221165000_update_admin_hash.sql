-- Update the admin user password hash to a valid argon2id hash for 'Admin123!'
UPDATE users 
SET password_hash = '$argon2id$v=19$m=19456,t=2,p=1$uXV90wKzlFyegiD2Q/sDdg$t6oo6g/J0VwRW4l3wwB6OEJIwf8bLoykRkbOcacV1CM' 
WHERE username = 'admin';
