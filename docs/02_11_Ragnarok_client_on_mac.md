การติดตั้ง Ragnarok Client บน Mac เพื่อเชื่อมต่อกับ rAthena Local Host ในปี 2026 มีความท้าทายหลักอยู่ที่ตัว Client เป็นไฟล์ .exe สำหรับ Windows และเรื่องสถาปัตยกรรมของ Apple Silicon (M1/M2/M3)

เนื่องจากเป็นการเล่นบน Local Host (เซิร์ฟเวอร์ตัวเอง) คุณจะไม่ติดปัญหาเรื่อง Anti-cheat (เช่น EAC หรือ Gepard) ที่มักจะบล็อกการเล่นบน Mac ครับ โดยมีวิธีหลักๆ ดังนี้:

1. ใช้ Whisky หรือ CrossOver (แนะนำสำหรับความลื่น)
วิธีนี้คือการสร้าง "Bottle" เพื่อจำลองสภาพแวดล้อม Windows ขึ้นมาบน macOS โดยตรง

ดาวน์โหลด: ติดตั้ง Whisky (ฟรีและใช้งานง่าย) หรือ CrossOver (เสียเงินแต่เสถียรกว่า)

สร้าง Bottle: สร้าง Bottle ใหม่ เลือกประเภทเป็น Windows 10 หรือ 11 (64-bit)

ติดตั้ง Library: แนะนำให้ติดตั้ง mfc42, vcrun2008, และ d3dx9 ผ่าน Winetricks (ในตัวโปรแกรมจะมีเมนูให้เลือก) เพื่อให้ตัวเกมรันกราฟิกได้ปกติ

Copy Client: นำโฟลเดอร์เกม Ragnarok ที่เตรียมไว้ (ที่มีไฟล์ sclientinfo.xml ชี้ไปที่ 127.0.0.1) มาวางใน C: drive ของ Bottle

รันเกม: เลือกไฟล์ opensetup.exe เพื่อตั้งค่าหน้าจอ จากนั้นรันตัวเกม (เช่น Ragexe.exe) ผ่านโปรแกรม Whisky/CrossOver

2. การตั้งค่า Network (Local Host)
เนื่องจากคุณรันทั้ง Server และ Client บน Mac เครื่องเดียวกัน:

Server Side: หากคุณรัน rAthena บน Mac (ผ่าน Docker หรือ Compile เอง) ตรวจสอบว่าในไฟล์ Config (char_athena.conf, map_athena.conf, login_athena.conf) ตั้งค่า IP เป็น 127.0.0.1

Client Side: แก้ไขไฟล์ sclientinfo.xml หรือ clientinfo.xml ในโฟลเดอร์ data/grf:

XML
<address>127.0.0.1</address>
<port>6900</port>
หมายเหตุ: หากใช้ Whisky/CrossOver บางครั้งมันจะมองเห็น IP ของตัวมันเองเป็นวงเครือข่ายจำลอง ถ้า 127.0.0.1 เชื่อมต่อไม่ได้ ให้ลองใช้ IP เครื่อง Mac จริงๆ (ดูจาก System Settings > Network)

ข้อควรระวังเพิ่มเติม
Graphic Error: หากเปิดเกมแล้วจอดำหรือ Error ให้ลองใช้ dgVoodoo2 (เลือกรันผ่าน Direct3D 11/12) ใส่ไฟล์ .dll ของมันลงในโฟลเดอร์เกม จะช่วยให้ Mac รุ่นใหม่รันเกมเก่าได้ลื่นขึ้นมาก

Packetver: ตรวจสอบให้แน่ใจว่า PACKETVER ในตอนที่ Compile rAthena ตรงกับเวอร์ชันของ Client (.exe) ที่คุณใช้ ไม่อย่างนั้นจะเด้งออกจากเกมตอนเลือกตัวละคร