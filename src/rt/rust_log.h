#ifndef RUST_LOG_H
#define RUST_LOG_H

#define DLOG(dom, mask, ...)                      \
  if ((dom)->get_log().is_tracing(mask)) {        \
      (dom)->log(mask, __VA_ARGS__);              \
  } else
#define LOG(task, mask, ...)                      \
  DLOG((task)->dom, mask, __VA_ARGS__)
#define LOG_I(task, mask, ...)                    \
  if ((task)->dom->get_log().is_tracing(mask)) {  \
      (task)->dom->get_log().reset_indent(0);     \
      (task)->dom->log(mask, __VA_ARGS__);        \
      (task)->dom->get_log().indent();            \
  } else
#define LOGPTR(dom, msg, ptrval)                  \
  DLOG(dom, rust_log::MEM, "%s 0x%" PRIxPTR, msg, ptrval)

class rust_dom;
class rust_task;

class rust_log {

public:
    rust_log(rust_srv *srv, rust_dom *dom);
    virtual ~rust_log();

    enum ansi_color {
        WHITE,
        RED,
        LIGHTRED,
        GREEN,
        LIGHTGREEN,
        YELLOW,
        LIGHTYELLOW,
        BLUE,
        LIGHTBLUE,
        MAGENTA,
        LIGHTMAGENTA,
        TEAL,
        LIGHTTEAL
    };

    enum log_type {
        ERR = 0x1,
        MEM = 0x2,
        COMM = 0x4,
        TASK = 0x8,
        DOM = 0x10,
        ULOG = 0x20,
        TRACE = 0x40,
        DWARF = 0x80,
        CACHE = 0x100,
        UPCALL = 0x200,
        TIMER = 0x400,
        GC = 0x800,
        STDLIB = 0x1000,
        SPECIAL = 0x2000,
        KERN = 0x4000,
        BT = 0x8000,
        ALL = 0xffffffff
    };

    void indent();
    void outdent();
    void reset_indent(uint32_t indent);
    void trace_ln(uint32_t thread_id, char *prefix, char *message);
    void trace_ln(rust_task *task, uint32_t type_bits, char *message);
    void trace_ln(rust_task *task, ansi_color color,
                  uint32_t type_bits, char *message);
    bool is_tracing(uint32_t type_bits);

private:
    rust_srv *_srv;
    rust_dom *_dom;
    uint32_t _type_bit_mask;
    bool _use_labels;
    bool _use_colors;
    uint32_t _indent;
    void trace_ln(rust_task *task, char *message);
};

inline bool
rust_log::is_tracing(uint32_t type_bits) {
    return type_bits & _type_bit_mask;
}

#endif /* RUST_LOG_H */
