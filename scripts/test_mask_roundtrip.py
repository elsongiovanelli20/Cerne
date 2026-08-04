import main

txt = ("O contratante Joao da Silva (CPF 111.444.777-35, "
       "e-mail joao.souza@exemplo.com.br, tel (11) 98765-4321) deve pagar em 30 dias.")

masked, mapping = main.mask_reversible(txt)
print("ORIGINAL :", txt)
print("MASCARADO:", masked, "   <-- ISTO e o que sai para o provedor externo")
print("MAPA LOCAL:", mapping)

# simula a resposta do modelo externo, que so viu os tokens
fake = "Segundo o texto, [PERSON_1] (CPF [BR_CPF_1], e-mail [EMAIL_ADDRESS_1]) paga em 30 dias."
print("RESP DO GLM:", fake, "   <-- o provedor NUNCA viu a PII")
print("DE-MASCARADO:", main.demask(fake, mapping), "   <-- o usuario ve o dado real de volta")
