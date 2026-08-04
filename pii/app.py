"""
Cerne — Serviço de PII Masking (camada L2 / Integration).

Microserviço que envolve o Microsoft Presidio configurado para PORTUGUÊS,
com reconhecedores nativos para PII brasileira (CPF e CNPJ validados por
dígito verificador, além de CEP). Expõe um único endpoint HTTP /mask.

Por que um microserviço próprio em vez da imagem pronta do Presidio:
  - controle total do idioma (pt) e dos reconhecedores brasileiros;
  - CPF/CNPJ não existem no Presidio de fábrica — aqui são validados por checksum;
  - API estável, sob nosso domínio, portável para a nuvem (build arm64 ok).
"""
import os
import re
from fastapi import FastAPI
from pydantic import BaseModel
from presidio_analyzer import AnalyzerEngine, PatternRecognizer, Pattern, RecognizerRegistry
from presidio_analyzer.predefined_recognizers import CreditCardRecognizer
from presidio_analyzer.nlp_engine import NlpEngineProvider
from presidio_anonymizer import AnonymizerEngine

LANG = os.getenv("PII_LANG", "pt")
# Modelo NER: _lg (grande) detecta nomes de pessoa muito melhor que _sm. Configurável por env.
SPACY_MODEL = os.getenv("SPACY_MODEL", "pt_core_news_lg")


# ---------- validadores de dígito verificador (BR) ----------
def cpf_is_valid(text: str) -> bool:
    d = re.sub(r"\D", "", text)
    if len(d) != 11 or d == d[0] * 11:
        return False
    for i in (9, 10):
        s = sum(int(d[n]) * ((i + 1) - n) for n in range(i))
        check = (s * 10) % 11 % 10
        if check != int(d[i]):
            return False
    return True


def cnpj_is_valid(text: str) -> bool:
    d = re.sub(r"\D", "", text)
    if len(d) != 14 or d == d[0] * 14:
        return False

    def dig(nums, weights):
        s = sum(int(n) * w for n, w in zip(nums, weights))
        r = s % 11
        return 0 if r < 2 else 11 - r

    w1 = [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]
    w2 = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2]
    return dig(d[:12], w1) == int(d[12]) and dig(d[:13], w2) == int(d[13])


# ---------- reconhecedores brasileiros ----------
class CpfRecognizer(PatternRecognizer):
    def __init__(self):
        super().__init__(
            supported_entity="BR_CPF",
            patterns=[Pattern("cpf", r"\b\d{3}\.?\d{3}\.?\d{3}-?\d{2}\b", 0.5)],
            context=["cpf", "contribuinte"],
            supported_language=LANG,
        )

    def validate_result(self, pattern_text):
        return cpf_is_valid(pattern_text)


class CnpjRecognizer(PatternRecognizer):
    def __init__(self):
        super().__init__(
            supported_entity="BR_CNPJ",
            patterns=[Pattern("cnpj", r"\b\d{2}\.?\d{3}\.?\d{3}/?\d{4}-?\d{2}\b", 0.5)],
            context=["cnpj", "empresa", "inscrição"],
            supported_language=LANG,
        )

    def validate_result(self, pattern_text):
        return cnpj_is_valid(pattern_text)


class CepRecognizer(PatternRecognizer):
    def __init__(self):
        super().__init__(
            supported_entity="BR_CEP",
            patterns=[Pattern("cep", r"\b\d{5}-?\d{3}\b", 0.3)],
            context=["cep", "endereço", "endereco"],
            supported_language=LANG,
        )


# Documentos brasileiros: score-base baixo + context words para evitar mascarar
# qualquer sequência de dígitos solta (só dispara forte perto do termo indicativo).
class RgRecognizer(PatternRecognizer):
    def __init__(self):
        super().__init__(
            supported_entity="BR_RG",
            patterns=[Pattern("rg", r"\b\d{1,2}\.?\d{3}\.?\d{3}-?[\dxX]\b", 0.25)],
            context=["rg", "identidade", "registro geral", "ssp"],
            supported_language=LANG,
        )


class TituloEleitorRecognizer(PatternRecognizer):
    def __init__(self):
        super().__init__(
            supported_entity="BR_TITULO_ELEITOR",
            patterns=[Pattern("titulo", r"\b\d{4}[\s.]?\d{4}[\s.]?\d{4}\b", 0.3)],
            context=["título", "titulo", "eleitor", "eleitoral", "zona", "seção"],
            supported_language=LANG,
        )


class CnhRecognizer(PatternRecognizer):
    def __init__(self):
        super().__init__(
            supported_entity="BR_CNH",
            patterns=[Pattern("cnh", r"\b\d{11}\b", 0.2)],
            context=["cnh", "habilitação", "habilitacao", "carteira de motorista", "renach"],
            supported_language=LANG,
        )


class PisRecognizer(PatternRecognizer):
    def __init__(self):
        super().__init__(
            supported_entity="BR_PIS",
            patterns=[Pattern("pis", r"\b\d{3}\.?\d{5}\.?\d{2}-?\d\b", 0.3)],
            context=["pis", "pasep", "nis", "nit"],
            supported_language=LANG,
        )


# ---------- engine ----------
nlp_engine = NlpEngineProvider(nlp_configuration={
    "nlp_engine_name": "spacy",
    "models": [{"lang_code": LANG, "model_name": SPACY_MODEL}],
}).create_engine()

registry = RecognizerRegistry(supported_languages=[LANG])
registry.load_predefined_recognizers(languages=[LANG], nlp_engine=nlp_engine)
registry.add_recognizer(CpfRecognizer())
registry.add_recognizer(CnpjRecognizer())
registry.add_recognizer(CepRecognizer())
registry.add_recognizer(RgRecognizer())
registry.add_recognizer(TituloEleitorRecognizer())
registry.add_recognizer(CnhRecognizer())
registry.add_recognizer(PisRecognizer())
registry.add_recognizer(CreditCardRecognizer(supported_language=LANG))  # cartão (Luhn), em pt

analyzer = AnalyzerEngine(nlp_engine=nlp_engine, registry=registry, supported_languages=[LANG])
anonymizer = AnonymizerEngine()

app = FastAPI(title="Cerne — PII Masking (L2)")


class MaskRequest(BaseModel):
    text: str
    language: str | None = None


@app.get("/health")
def health():
    return {"status": "ok", "language": LANG, "model": SPACY_MODEL}


@app.post("/mask")
def mask(req: MaskRequest):
    lang = req.language or LANG
    results = analyzer.analyze(text=req.text, language=lang)
    anon = anonymizer.anonymize(text=req.text, analyzer_results=results)  # substitui por <TIPO>
    return {
        "masked_text": anon.text,
        "entity_types": sorted({r.entity_type for r in results}),
        "entities_found": [
            {"type": r.entity_type, "start": r.start, "end": r.end, "score": round(r.score, 2)}
            for r in results
        ],
    }
