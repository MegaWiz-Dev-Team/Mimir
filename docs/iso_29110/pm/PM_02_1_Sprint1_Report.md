# PM-02.1: Sprint 1 Status Report (Security Foundation & IAM)

**Project Name:** Project Mimir
**Sprint:** Sprint 1
**Status:** Completed
**Date:** 2026-02-22

---

## 1. ขอบเขตของ Sprint 1 (Sprint Scope)
ระบบ Security Foundation & IAM ประกอบด้วย:
- **Backend:** สร้าง CRUD APIs สำหรับ Users, Tenants และ Roles
- **Backend:** ทำระบบ Authentication ด้วย Argon2id และ JWT (`tenant_id`)
- **Backend:** Implement `tenant_auth_middleware`
- **Frontend:** สร้าง User Management Dashboard และ Interactive Data Table
- **Frontend:** สร้างระบบฟอร์ม Add/Edit User (Sliding Drawer / Modal)

## 2. สรุปผลการทดสอบ (Testing Verification Summary)
อ้างอิงจากแผนการทดสอบ `SI_04_1_Sprint1_TestScript.md` ทุกฟีเจอร์ผ่านการทดสอบสมบูรณ์ 100%:

- **TS-1.1 (GitHub Sync & Docs):** Pass (เตรียมโครงสร้างและ Issue Tracking)
- **TS-1.2 (Login Failed):** Pass (ระบบจับรหัสผ่านผิด และแสดง Error "Login failed" ได้ถูกต้อง)
- **TS-1.3 (Read Users):** Pass (ตาราง Users แสดงข้อมูลได้ถูกต้อง หลังจากปรับ API Path เป็น `/api/v1`)
- **TS-1.4 (Create User):** Pass (ฟอร์มทำงานถูกต้อง จัดการ State ไม่ให้ข้อมูลซ้อนกัน และบันทึกลง Database ได้สำเร็จพร้อม Role `EDITOR`)
- **TS-1.5 (Delete User):** Pass (ลบผู้ใช้งาน `testuser_new` ได้จริง พร้อม Modal ยืนยันก่อนลบ)
- **TS-1.6 (Auth Middleware):** Pass (ทดสอบยิง API ค้นหา Users แบบไม่แนบ JWT โดนบล็อกด้วย HTTP 401 Unauthorized ทันที)

## 3. GitHub Synchronization & Traceability
ความคืบหน้าและการแก้ไขบัคทั้งหมดถูกเชื่อมโยงและปิดด้วย Pull Request (PR) อย่างสมบูรณ์:
- **[Sprint 1] TS-1.1:** PR #10 (Merge `fix/ts-1.1-login-dashboard`)
- **[Sprint 1] TS-1.2:** PR #10 (Merge `test/ts-1.2-login-failed` integrated into main fixes)
- **[Sprint 1] TS-1.3:** PR #10 (Merge `test/ts-1.3-read-users` integrated into main fixes)
- **[Sprint 1] TS-1.4:** PR #12 (Merge `test/ts-1.4-create-user`)
- **[Sprint 1] TS-1.5:** PR #14 (Merge `test/ts-1.5-delete-user`)
- **[Sprint 1] TS-1.6:** PR #16 (Merge `test/ts-1.6-auth-middleware`)

## 4. ปัญหาที่พบและวิธีแก้ไข (Issues & Resolutions)
1. **Frontend API Path Mismatch:** 
   - *ปัญหา:* Frontend ยิงไปที่ `/api/iam` แต่ Backend ตั้ง Route ไว้ที่ `/api/v1/iam` (เกิด 404)
   - *แก้ปัญหา:* อัปเดตไฟล์ `api.ts` ใน Frontend ให้เรียก Endpoint ได้ถูกต้อง
2. **Frontend Form Auto-fill Conflict:** 
   - *ปัญหา:* Chrome Auto-fill เข้ามาแทรกแซงและ state ใน React รวม string ซ้อนกัน
   - *แก้ปัญหา:* เพิ่ม `autoComplete="new-username"` และ สั่งล้าง input ให้ชัดเจนก่อนพิมพ์
3. **Database Enum Case Sensitivity:** 
   - *ปัญหา:* DB บังคับ Role เป็น `ADMIN`, `EDITOR`, `VIEWER` (uppercase) แต่ตาราง UI ส่งเป็นตัวเล็ก เกิด HTTP 500
   - *แก้ปัญหา:* ปรับ React Select Components ให้ส่งค่า Enum เป็น Uppercase ตามที่ DB ต้องการ
4. **Auth Middleware Bypass:** 
   - *ปัญหา:* Middleware ของ Backend มีโค้ด Legacy ข้ามการเช็ค JWT ถ้าไม่มี Header ส่งมา (ได้สิทธิ์ Admin อัตโนมัติ)
   - *แก้ปัญหา:* รื้อโค้ด Legacy ออกและบังคับใช้ Strict JWT Authentication เพื่อความปลอดภัยสูงสุด (แก้ไขใน TS-1.6)

---
*บันทึกโดย: AI Assistant (ตามมาตรฐาน ISO/IEC 29110 หมวด PM-02)*
