#include <libheif/heif.h>
#include <cstdint>
#include <memory>
#include <new>

// Only this small, fixed C ABI crosses the Rust/C++ boundary. No C++ objects,
// exceptions, or allocator ownership cross it.
struct LumenHeicImage {
    const uint8_t* pixels;
    void* owner;
    size_t stride;
    uint32_t width;
    uint32_t height;
};

struct OwnedImage {
    std::unique_ptr<heif_context, decltype(&heif_context_free)> context;
    std::unique_ptr<heif_image, decltype(&heif_image_release)> image;
};

static bool valid_size(int width, int height, uint64_t max_pixels, uint32_t max_side) {
    return width > 0 && height > 0 && uint32_t(width) <= max_side &&
           uint32_t(height) <= max_side && uint64_t(width) * uint64_t(height) <= max_pixels;
}

static int error_code(heif_error error) {
    if (error.code == heif_error_Unsupported_feature ||
        error.code == heif_error_Unsupported_filetype ||
        error.code == heif_error_Plugin_loading_error) return 3;
    if (error.code == heif_error_Memory_allocation_error ||
        error.subcode == heif_suberror_Security_limit_exceeded) return 2;
    return 1;
}

extern "C" int lumen_heic_decode(const uint8_t* data, size_t length,
                                uint64_t max_pixels, uint64_t max_bytes,
                                uint32_t max_side, LumenHeicImage* output) noexcept {
    *output = {};
    try {
        // C++ static initialization serializes the one process-wide init call.
        // The library remains initialized for the lifetime of the application.
        static const heif_error initialized = heif_init(nullptr);
        if (initialized.code != heif_error_Ok) return error_code(initialized);
        std::unique_ptr<heif_context, decltype(&heif_context_free)>
            context(heif_context_alloc(), heif_context_free);
        if (!context) return 2;
        auto* limits = heif_context_get_security_limits(context.get());
        limits->max_image_size_pixels = max_pixels;
        limits->max_memory_block_size = max_bytes;
        limits->max_total_memory = max_bytes;
        heif_context_set_max_decoding_threads(context.get(), 2);
        auto error = heif_context_read_from_memory_without_copy(context.get(), data, length, nullptr);
        if (error.code != heif_error_Ok) return error_code(error);
        heif_image_handle* raw_handle = nullptr;
        error = heif_context_get_primary_image_handle(context.get(), &raw_handle);
        std::unique_ptr<heif_image_handle, decltype(&heif_image_handle_release)>
            handle(raw_handle, heif_image_handle_release);
        if (error.code != heif_error_Ok) return error_code(error);
        if (!valid_size(heif_image_handle_get_width(handle.get()),
                        heif_image_handle_get_height(handle.get()), max_pixels, max_side)) return 2;
        std::unique_ptr<heif_decoding_options, decltype(&heif_decoding_options_free)>
            options(heif_decoding_options_alloc(), heif_decoding_options_free);
        if (!options) return 2;
        options->num_codec_threads = 1;
        options->convert_hdr_to_8bit = 1;
        heif_image* raw_image = nullptr;
        error = heif_decode_image(handle.get(), &raw_image, heif_colorspace_RGB,
                                  heif_chroma_interleaved_RGBA, options.get());
        std::unique_ptr<heif_image, decltype(&heif_image_release)>
            image(raw_image, heif_image_release);
        if (error.code != heif_error_Ok) return error_code(error);
        const int width = heif_image_get_width(image.get(), heif_channel_interleaved);
        const int height = heif_image_get_height(image.get(), heif_channel_interleaved);
        if (!valid_size(width, height, max_pixels, max_side)) return 2;
        size_t stride = 0;
        const auto* pixels = heif_image_get_plane_readonly2(image.get(), heif_channel_interleaved, &stride);
        if (!pixels || stride < size_t(width) * 4 || stride > max_bytes / size_t(height)) return 2;
        output->pixels = pixels;
        output->stride = stride;
        output->width = uint32_t(width);
        output->height = uint32_t(height);
        output->owner = new OwnedImage{std::move(context), std::move(image)};
        return 0;
    } catch (const std::bad_alloc&) {
        return 2;
    } catch (...) {
        return 1;
    }
}

extern "C" void lumen_heic_free(void* owner) noexcept {
    delete static_cast<OwnedImage*>(owner);
}
