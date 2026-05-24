/*
 * m72: tiny C harness exercising lotus_tcp_* from the runtime.
 * Built by tests/transport_tcp.rs into a single binary that is
 * then exec'd twice — once as listener, once as connector — to
 * verify framed-message round-trip over an AF_INET SOCK_STREAM
 * pair. Mirrors transport_driver.c (the AF_UNIX SEQPACKET
 * harness) but speaks TCP.
 *
 * Forward-declare the TCP transport surface here rather than
 * carry a runtime header file: m72 keeps the C-runtime
 * install-free and the surface is small enough that forward
 * decls + linking lotus_arena.c into the test binary is the
 * lightest path.
 *
 * argv:
 *   listen  <host> <port>           -> recv messages until -1, write each to stdout
 *   connect <host> <port> <bytes>+  -> send each remaining argv as one message
 *
 * Multi-message support lets a single driver invocation prove
 * that the length-prefix framer correctly delineates back-to-back
 * sends — the boundary-preservation regression that TCP without
 * framing would silently fail.
 */

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>

#define LOTUS_TCP_LISTEN  0
#define LOTUS_TCP_CONNECT 1

typedef struct lotus_tcp lotus_tcp_t;

lotus_tcp_t *lotus_tcp_create(const char *host, uint16_t port, int role);
int          lotus_tcp_send(lotus_tcp_t *t, const void *buf, size_t len);
ssize_t      lotus_tcp_recv(lotus_tcp_t *t, void *buf, size_t cap);
void         lotus_tcp_destroy(lotus_tcp_t *t);

#define BUF_CAP (64 * 1024)

static int run_listen(const char *host, uint16_t port) {
    lotus_tcp_t *t = lotus_tcp_create(host, port, LOTUS_TCP_LISTEN);
    if (!t) {
        fprintf(stderr, "listener: create failed\n");
        return 1;
    }
    char buf[BUF_CAP];
    /* Loop receiving framed messages until the peer closes the
     * stream (recv returns -1 with errno=EIO from our adapter).
     * Each message is written to stdout followed by a "\n----\n"
     * delimiter so the test can split them apart unambiguously
     * even if a payload contains its own newlines. */
    for (;;) {
        ssize_t n = lotus_tcp_recv(t, buf, sizeof(buf));
        if (n < 0) {
            /* either clean EOF (peer closed) or a real error;
             * either way, stop. The test asserts on count + bytes. */
            break;
        }
        if (fwrite(buf, 1, (size_t)n, stdout) != (size_t)n) {
            fprintf(stderr, "listener: stdout write short\n");
            lotus_tcp_destroy(t);
            return 1;
        }
        fputs("\n----\n", stdout);
    }
    fflush(stdout);
    lotus_tcp_destroy(t);
    return 0;
}

static int run_connect(const char *host, uint16_t port, int argc, char **argv) {
    lotus_tcp_t *t = lotus_tcp_create(host, port, LOTUS_TCP_CONNECT);
    if (!t) {
        fprintf(stderr, "connector: create failed\n");
        return 1;
    }
    for (int i = 0; i < argc; i++) {
        size_t len = strlen(argv[i]);
        if (lotus_tcp_send(t, argv[i], len) != 0) {
            fprintf(stderr, "connector: send[%d] failed\n", i);
            lotus_tcp_destroy(t);
            return 1;
        }
    }
    /* Closing the connection signals end-of-stream to the
     * listener; the next recv there returns -1 with errno=EIO
     * and the listener's loop exits cleanly. */
    lotus_tcp_destroy(t);
    return 0;
}

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr,
                "usage: %s listen <host> <port> | connect <host> <port> <bytes>+\n",
                argv[0]);
        return 2;
    }
    const char *host = argv[2];
    int   port_int  = atoi(argv[3]);
    if (port_int < 0 || port_int > 65535) {
        fprintf(stderr, "invalid port %s\n", argv[3]);
        return 2;
    }
    uint16_t port = (uint16_t)port_int;
    if (strcmp(argv[1], "listen") == 0) {
        return run_listen(host, port);
    }
    if (strcmp(argv[1], "connect") == 0) {
        if (argc < 5) {
            fprintf(stderr, "connect: need at least one <bytes> argument\n");
            return 2;
        }
        return run_connect(host, port, argc - 4, argv + 4);
    }
    fprintf(stderr, "unknown role: %s\n", argv[1]);
    return 2;
}
