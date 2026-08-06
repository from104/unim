/*
 * UNIM XIM Test - Log Module
 *
 * 링 버퍼 기반 이벤트 로그 관리
 */

#include "app.h"
#include "log.h"

void log_init(void) {
    app.log_count = 0;
    for (int i = 0; i < MAX_LOG_LINES; i++) {
        app.log_buffer[i][0] = '\0';
    }
}

void log_add(const char *format, ...) {
    va_list args;
    va_start(args, format);

    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    struct tm *tm_info = localtime(&ts.tv_sec);
    char hms[16];
    strftime(hms, sizeof(hms), "%H:%M:%S", tm_info);
    char timestamp[32];
    snprintf(timestamp, sizeof(timestamp), "%s.%03ld", hms, ts.tv_nsec / 1000000);

    char message[200];
    vsnprintf(message, sizeof(message), format, args);
    va_end(args);

    /* 링 버퍼: 가득 차면 한 칸씩 밀기 */
    if (app.log_count >= MAX_LOG_LINES) {
        for (int i = 0; i < MAX_LOG_LINES - 1; i++) {
            strcpy(app.log_buffer[i], app.log_buffer[i + 1]);
        }
        app.log_count = MAX_LOG_LINES - 1;
    }

    snprintf(app.log_buffer[app.log_count], sizeof(app.log_buffer[0]),
             "[%s] %s", timestamp, message);
    app.log_count++;

    /* 콘솔에도 출력 */
    printf("[%s] %s\n", timestamp, message);
}
