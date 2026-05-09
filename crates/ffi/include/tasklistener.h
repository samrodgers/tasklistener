/* TaskListener C ABI.
 * Hand-maintained header consumed by the Swift package and the C# P/Invoke
 * bindings. Keep in lockstep with crates/ffi/src/lib.rs.
 *
 * Strings returned by these functions are heap-allocated and MUST be freed
 * with tl_string_free. Strings passed in are copied — caller retains ownership.
 */
#ifndef TASKLISTENER_H
#define TASKLISTENER_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void (*tl_event_cb)(const char *json, void *ctx);

/* Engine */
int32_t tl_start(const char *db_path);
int32_t tl_subscribe(tl_event_cb cb, void *ctx);
void    tl_string_free(char *s);

/* Tasks */
char   *tl_capture_manual(const char *text);
char   *tl_list_tasks(int32_t include_done, int64_t limit);
char   *tl_get_task(const char *id);
int32_t tl_update_task_text(const char *id, const char *text);
int32_t tl_set_task_status(const char *id, const char *status);
int32_t tl_delete_task(const char *id);

/* Providers */
int32_t tl_set_provider(const char *config_json, const char *token);
char   *tl_list_providers(void);
int32_t tl_delete_provider(const char *id);
char   *tl_list_targets(const char *provider_id);
int32_t tl_push_now(const char *task_id, const char *provider_id);

/* Front-end-handled push (e.g. Apple Reminders via EventKit). Pass either
 * external_id (success) or error (failure). */
int32_t tl_record_external_push(const char *task_id,
                                const char *provider_id,
                                const char *external_id,
                                const char *external_url,
                                const char *error);

/* Diagnostics */
int32_t tl_audio_is_real(void);

#ifdef __cplusplus
}
#endif
#endif
