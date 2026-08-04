FROM python:3.11-slim
WORKDIR /srv
COPY requirements.txt .
# pt_core_news_lg: modelo NER GRANDE (detecta nomes de pessoa muito melhor que o _sm).
# ARG permite trocar no build (ex.: _md p/ menos RAM, _sm p/ mínimo).
ARG SPACY_MODEL=pt_core_news_lg
RUN pip install --no-cache-dir -r requirements.txt \
 && python -m spacy download ${SPACY_MODEL}
COPY app.py .
EXPOSE 8080
CMD ["uvicorn", "app:app", "--host", "0.0.0.0", "--port", "8080"]
