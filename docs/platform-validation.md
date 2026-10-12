# Validación de paquetes por plataforma

Los workflows `CI` y `Release packages` generan los artefactos para Linux x86_64 y Windows x86_64. El paquete se extrae antes de ejecutar sus pruebas, por lo que la aceptación se aplica al ejecutable distribuible y a los recursos incluidos.

El destino Windows es `x86_64-pc-windows-msvc`. [`.cargo/config.toml`](../.cargo/config.toml) activa `+crt-static` para ese destino, de modo que el runtime de MSVC se enlaza estáticamente. El manifiesto de cada paquete declara su política de dependencias externas y la huella SHA-256 del árbol visible para Git, junto con el indicador `source_dirty` y el commit base. `scripts/check-package.mjs` valida esos campos y, para Windows, inspecciona la tabla PE de imports para rechazar dependencias dinámicas del CRT de MSVC.

La configuración del workflow ya está preparada, pero la validación formal de Windows queda pendiente de una ejecución completada en GitHub Actions para el commit que incorpora estos cambios. El parser PE se probó localmente con un ejecutable sintético; eso no valida el binario Windows real. La validación Linux local tampoco sustituye esa evidencia. No se afirma aquí que el paquete Windows ya se haya ejecutado.

La protección de la rama principal también requiere una acción manual del administrador del repositorio. Debe exigir `Verify (ubuntu-24.04)`, `Verify (windows-2022)` y `Compare Linux and Windows B0 and E2-B0 output`. Estos archivos no cambian la configuración remota del repositorio.
