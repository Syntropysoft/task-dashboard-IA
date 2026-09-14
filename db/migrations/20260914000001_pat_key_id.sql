-- PAT con key_id + secret (decisión 2026-09-14): el id de la fila es el key_id (viaja en claro
-- dentro del token, `tdp_<key_id>.<secret>`); la columna guarda el hash del secret solamente.
-- Renombrar deja claro qué se hashea. Nunca se editó la migración inicial: esta va aparte.
alter table access_tokens rename column token_hash to secret_hash;
