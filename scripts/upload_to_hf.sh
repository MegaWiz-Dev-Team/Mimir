#!/usr/bin/env bash
# สคริปต์สำหรับอัปโหลด Asgard LoRA Adapters ขึ้น Hugging Face
# กรุณาแน่ใจว่าได้ทำการ login ผ่าน CLI แล้ว (ใช้คำสั่ง: hf auth login)

set -e

# ตั้งค่าเริ่มต้น
HF_NAMESPACE="MegawizCo"
BASE_REPO_NAME="asgard-medical-gemma-26b-lora"
LOCAL_ADAPTER_DIR="/tmp/lora_mvp_adapter" # Path ของ Adapter ที่ต้องการอัปโหลด
MODEL_VERSION="mvp" # เช่น mvp, phase2, phase2c

echo "============================================================"
echo "🚀 เตรียมอัปโหลด LoRA Adapter ขึ้น Hugging Face"
echo "============================================================"

# 1. รับค่าจากผู้ใช้ (สามารถกด Enter เพื่อใช้ค่าเริ่มต้นได้)
read -p "กรอก Hugging Face Username หรือ Organization Name [$HF_NAMESPACE]: " input_user
HF_NAMESPACE=${input_user:-$HF_NAMESPACE}

read -p "กรอกชื่อ Repository [$BASE_REPO_NAME]: " input_repo
BASE_REPO_NAME=${input_repo:-$BASE_REPO_NAME}

read -p "กรอก Path ของ LoRA Adapter ที่จะอัปโหลด [$LOCAL_ADAPTER_DIR]: " input_dir
LOCAL_ADAPTER_DIR=${input_dir:-$LOCAL_ADAPTER_DIR}

REPO_ID="${HF_NAMESPACE}/${BASE_REPO_NAME}-${MODEL_VERSION}"

echo ""
echo "ตรวจสอบข้อมูล:"
echo "- Local Path: $LOCAL_ADAPTER_DIR"
echo "- Target HF Repo: $REPO_ID"
echo ""
read -p "ยืนยันการอัปโหลด? (y/n): " confirm
if [[ $confirm != [yY] ]]; then
    echo "ยกเลิกการอัปโหลด"
    exit 1
fi

# 2. ตรวจสอบว่ามี Directory อยู่จริงหรือไม่
if [ ! -d "$LOCAL_ADAPTER_DIR" ]; then
    echo "❌ ไม่พบ Directory: $LOCAL_ADAPTER_DIR"
    exit 1
fi

# 3. อัปโหลดผ่าน hf cli
echo "กำลังดำเนินการอัปโหลดไปที่ https://huggingface.co/$REPO_ID ..."

# อัปโหลดไฟล์ใน Directory (ระบบจะสร้าง repo ให้อัตโนมัติถ้ายังไม่มี)
hf upload "$REPO_ID" "$LOCAL_ADAPTER_DIR" . --repo-type model

echo "✅ อัปโหลดเสร็จสิ้น!"
echo "คุณสามารถดูโมเดลของคุณได้ที่: https://huggingface.co/$REPO_ID"
