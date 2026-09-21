/* as.exe shim: dlltool spawns `as` to assemble import stubs, but the
 * rust-mingw self-contained bundle ships no assembler. This wrapper
 * re-execs the bundled gcc with -c (assemble-only), which is flag-
 * compatible with how dlltool invokes `as`. See docs/DECISIONS.md ADR-006. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <process.h>

int main(int argc, char **argv) {
    const char *gcc = "x86_64-w64-mingw32-gcc.exe";
    char **na = (char **)malloc(sizeof(char *) * (argc + 3));
    if (!na) return 1;
    na[0] = (char *)gcc;
    na[1] = "-c";
    for (int i = 1; i < argc; i++) na[i + 1] = argv[i];
    na[argc + 1] = NULL;
    intptr_t rc = _spawnvp(_P_WAIT, gcc, (const char *const *)na);
    if (rc == -1) {
        perror("spawn gcc");
        return 1;
    }
    return (int)rc;
}
