import httpx
txt = ("Contratante Joao da Silva, CPF 111.444.777-35, "
       "email joao.souza@exemplo.com.br, telefone (11) 98765-4321, "
       "CNPJ 11.222.333/0001-81, CEP 01310-100. "
       "Um CPF invalido 123.456.789-00 nao deve ser mascarado.")
r = httpx.post("http://pii:8080/mask", json={"text": txt}, timeout=60).json()
print("MASCARADO:", r["masked_text"])
print("TIPOS:", r["entity_types"])
