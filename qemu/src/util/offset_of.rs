#[cfg(not(has_offset_of))]
#[macro_export]
macro_rules! offset_of {
    ($Container:ty, $field:ident) => {
        <$Container>::offset_to.$field
    };
}

/// A wrapper for struct declarations, that allows using `offset_of!` in
/// versions of Rust prior to 1.77
#[macro_export]
macro_rules! with_offsets {
    // source: https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=10a22a9b8393abd7b541d8fc844bc0df
    // used under MIT license with permission of Yandros aka Daniel Henry-Mantilla
    (
        #[repr(C)]
        $(#[$struct_meta:meta])*
        $struct_vis:vis
        struct $StructName:ident {
            $(
                $(#[$field_meta:meta])*
                $field_vis:vis
                $field_name:ident : $field_ty:ty
            ),*
            $(,)?
        }
    ) => (
        #[repr(C)]
        $(#[$struct_meta])*
        $struct_vis
        struct $StructName {
            $(
                $(#[$field_meta])*
                $field_vis
                $field_name : $field_ty ,
            )*
        }

        #[cfg(not(has_offset_of))]
        #[allow(nonstandard_style)]
        const _: () = {
            pub
            struct StructOffsets {
                $(
                    $field_vis
                    $field_name: usize,
                )*
            }
            struct Helper;
            impl $StructName {
                pub
                const offset_to: StructOffsets = StructOffsets {
                    $(
                        $field_name: Helper::$field_name,
                    )*
                };
            }
            const END_OF_PREV_FIELD: usize = 0;
            $crate::with_offsets! {
                @names [ $($field_name)* ]
                @tys [ $($field_ty ,)*]
            }
        };
    );

    (
        @names []
        @tys []
    ) => ();

    (
        @names [$field_name:ident $($other_names:tt)*]
        @tys [$field_ty:ty , $($other_tys:tt)*]
    ) => (
        impl Helper {
            const $field_name: usize = {
                let align =
                    std::mem::align_of::<$field_ty>()
                ;
                let trail =
                    END_OF_PREV_FIELD % align
                ;
                0   + END_OF_PREV_FIELD
                    + (align - trail)
                        * [1, 0][(trail == 0) as usize]
            };
        }
        const _: () = {
            const END_OF_PREV_FIELD: usize =
                Helper::$field_name +
                std::mem::size_of::<$field_ty>()
            ;
            $crate::with_offsets! {
                @names [$($other_names)*]
                @tys [$($other_tys)*]
            }
        };
    );
}
