FROM python:3.12-slim

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Clone PageIndex lib
RUN pip install --no-cache-dir git+https://github.com/VectifyAI/PageIndex.git || true

COPY . .

EXPOSE 8600

CMD ["python", "app.py"]
