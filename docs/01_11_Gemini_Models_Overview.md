# Gemini Models Overview

เอกสารนี้สรุปข้อมูลโมเดลในตระกูล Gemini รุ่นต่างๆ ที่รองรับสำหรับการใช้งานผ่าน Cloud API เพื่อเป็นข้อมูลอ้างอิงในการพัฒนาและเลือกใช้โมเดลที่เหมาะสมกับความต้องการของโปรเจกต์

---

## 1. Gemini 3 (Next Generation)

โมเดลรุ่นล่าสุดที่เน้นความฉลาดขั้นสูงและการประมวลผลแบบ Multimodal

*   **[Gemini 3 Pro](https://ai.google.dev/gemini-api/docs/models/gemini-3-pro-preview)**: โมเดลที่ฉลาดที่สุดในโลกสำหรับการทำ Multimodal Understanding และการใช้เหตุผล (State-of-the-art reasoning)
*   **[Gemini 3 Flash](https://ai.google.dev/gemini-api/docs/models/gemini-3-flash-preview)**: ให้ประสิทธิภาพระดับ Frontier เทียบเท่าโมเดลขนาดใหญ่ ในราคาและเวลาประมวลผลที่ต่ำกว่า
*   **[Nano Banana Pro](https://ai.google.dev/gemini-api/docs/models/gemini-3-pro-image-preview)**: โมเดลการสร้างและแก้ไขภาพขั้นสูงที่เน้นความเข้าใจในบริบทเพื่อการสร้างภาพแบบ Native

---

## 2. Gemini 2.5 Flash Family

เน้นความเร็ว ความคุ้มค่า และการใช้งานจริง (Best price-performance)

*   **[Gemini 2.5 Flash](https://ai.google.dev/gemini-api/docs/models/gemini-2.5-flash)**: เหมาะสำหรับงานที่ต้องการ Low-latency และ High-volume โดยยังคงความสามารถในการใช้เหตุผล
*   **[Nano Banana](https://ai.google.dev/gemini-api/docs/models/gemini-2.5-flash-image)**: การสร้างและแก้ไขภาพระดับสูงที่ออกแบบมาเพื่อ Workflow การสร้างสรรค์ที่รวดเร็ว
*   **[Gemini 2.5 Flash Live Preview](https://ai.google.dev/gemini-api/docs/models/gemini-2.5-flash-native-audio-preview-12-2025)**: ปรับแต่งมาเพื่อการสื่อสารแบบ Real-time (Native audio streaming)
*   **[Gemini 2.5 Flash TTS Preview](https://ai.google.dev/gemini-api/docs/models/gemini-2.5-flash-preview-tts)**: การสร้างเสียงจากข้อความ (Text-to-speech) ที่ควบคุมสไตล์และจังหวะการพูดได้แม่นยำ
*   **[Gemini 2.5 Flash-Lite](https://ai.google.dev/gemini-api/docs/models/gemini-2.5-flash-lite)**: โมเดล Multimodal ที่เร็วและประหยัดที่สุดในตระกูล 2.5

---

## 3. Gemini 2.5 Pro Family

โมเดลสำหรับงานซับซ้อนที่ต้องการความแม่นยำสูง

*   **[Gemini 2.5 Pro](https://ai.google.dev/gemini-api/docs/models/gemini-2.5-pro)**: โมเดลที่ก้าวหน้าที่สุดสำหรับงานซับซ้อน (Deep reasoning) และการเขียนโปรแกรม (Coding capabilities)
*   **[Gemini 2.5 Pro TTS Preview](https://ai.google.dev/gemini-api/docs/models/gemini-2.5-pro-preview-tts)**: การสังเคราะห์เสียงคุณภาพสูง (High-fidelity) เหมาะสำหรับ Podcast และ Audiobook

---

## 4. Audio & Generative Media Models

โมเดลเฉพาะทางด้านเสียงและสื่อผสม

### Audio Models
*   **[Lyria Experimental](https://ai.google.dev/gemini-api/docs/models/lyria-realtime-exp)**: โมเดลสร้างเพลงคุณภาพสูงที่ควบคุมเครื่องดนตรี, BPM และองค์ประกอบเพลงได้อย่างละเอียด

### Generative Media Models
*   **[Veo 3.1 Preview](https://ai.google.dev/gemini-api/docs/models/veo-3.1-generate-preview)**: การสร้างวิดีโอระดับ Cinematic พร้อมการซิงค์เสียงแบบ Native
*   **[Imagen 4](https://ai.google.dev/gemini-api/docs/models/imagen)**: โมเดล Text-to-image ที่รองรับการสร้างภาพระดับ 2K ด้วยความรวดเร็ว

---

## 5. Agentic & Specialized Models

โมเดลที่ออกแบบมาเพื่อการทำงานอัตโนมัติหรืองานเฉพาะด้าน

*   **[Computer Use Preview](https://ai.google.dev/gemini-api/docs/models/gemini-2.5-computer-use-preview-10-2025)**: สามารถ "เห็น" หน้าจอดิจิทัลและควบคุม UI (Click, Type, Navigate) เพื่อทำงานแทนมนุษย์
*   **[Gemini Deep Research Preview](https://ai.google.dev/gemini-api/docs/models/deep-research-pro-preview-12-2025)**: โมเดล Agentic ที่สามารถวางแผนและทำวิจัยจากข้อมูลนับร้อยแห่ง พร้อมเขียนรายงานที่อ้างอิงแหล่งข้อมูล
*   **[Gemini Embeddings](https://ai.google.dev/gemini-api/docs/models/gemini-embedding-001)**: สำหรับงาน Semantic Search, Text Classification และระบบ RAG (Vector representations)
*   **[Gemini Robotics Preview](https://ai.google.dev/gemini-api/docs/models/gemini-robotics-er-1.5-preview)**: โมเดลสำหรับงานหุ่นยนต์ที่เข้าใจพื้นที่ทางกายภาพ

---

## 6. Model Version Name Patterns

Gemini แบ่งรุ่นออกเป็น 4 ประเภทหลัก:

1.  **Stable**: สำหรับการใช้งานใน Production (เช่น `gemini-2.5-flash`)
2.  **Preview**: รุ่นทดลองก่อนใช้จริง อาจมีการเก็บค่าบริการ (เช่น `gemini-2.5-flash-preview-09-2025`)
3.  **Latest**: ชี้ไปยังเวอร์ชันล่าสุดของโมเดลนั้นๆ (Stable, Preview หรือ Experimental)
4.  **Experimental**: สำหรับการทดสอบฟีเจอร์ใหม่ ไม่แนะนำให้ใช้ใน Production

> [!WARNING]
> **Deprecated Models:** โมเดลตระกูล Gemini 2.0 (Flash และ Flash-Lite) กำลังจะถูกยกเลิก แนะนำให้ย้ายไปใช้ Gemini 2.5 แทน

---

*ดูข้อมูลเพิ่มเติมเรื่องการยกเลิกโมเดลได้ที่ [Gemini deprecations](https://ai.google.dev/gemini-api/docs/deprecations)*
