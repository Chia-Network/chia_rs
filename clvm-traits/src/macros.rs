#[macro_export]
macro_rules! clvm_list {
    () => {
        ()
    };
    ( $first:expr $( , $rest:expr )* $(,)? ) => {
        ($first, $crate::clvm_list!( $( $rest ),* ))
    };
}

#[macro_export]
macro_rules! clvm_tuple {
    ( $first:expr $(,)? ) => {
        $first
    };
    ( $first:expr $( , $rest:expr )* $(,)? ) => {
        ($first, $crate::clvm_tuple!( $( $rest ),* ))
    };
}

#[macro_export]
macro_rules! clvm_quote {
    ( $value:expr ) => {
        (1, $value)
    };
}

#[macro_export]
macro_rules! clvm_curried_args {
    () => {
        1
    };
    ( $first:expr $( , $rest:expr )* $(,)? ) => {
        (4, ($crate::clvm_quote!($first), ($crate::clvm_curried_args!( $( $rest ),* ), ())))
    };
}

#[macro_export]
macro_rules! match_list {
    () => {
        ()
    };
    ( $first:ty $( , $rest:ty )* $(,)? ) => {
        ($first, $crate::match_list!( $( $rest ),* ))
    };
}

#[macro_export]
macro_rules! match_tuple {
    ( $first:ty $(,)? ) => {
        $first
    };
    ( $first:ty $( , $rest:ty )* $(,)? ) => {
        ($first, $crate::match_tuple!( $( $rest ),* ))
    };
}

#[macro_export]
macro_rules! match_quote {
    ( $type:ty ) => {
        ($crate::MatchByte::<1>, $type)
    };
}

#[macro_export]
macro_rules! match_curried_args {
    () => {
        $crate::MatchByte::<1>
    };
    ( $first:ty $( , $rest:ty )* $(,)? ) => {
        (
            $crate::MatchByte::<4>,
            (
                $crate::match_quote!($first),
                ($crate::match_curried_args!( $( $rest ),* ), ()),
            ),
        )
    };
}

#[macro_export]
macro_rules! destructure_list {
    () => {
        _
    };
    ( $first:pat $( , $rest:pat )* $(,)? ) => {
        ($first, $crate::destructure_list!( $( $rest ),* ))
    };
}

#[macro_export]
macro_rules! destructure_tuple {
    ( $first:pat $(,)? ) => {
        $first
    };
    ( $first:pat $( , $rest:pat )* $(,)? ) => {
        ($first, $crate::destructure_tuple!( $( $rest ),* ))
    };
}

#[macro_export]
macro_rules! destructure_quote {
    ( $name:pat ) => {
        (_, $name)
    };
}

#[macro_export]
macro_rules! destructure_curried_args {
    () => {
        _
    };
    ( $first:pat $( , $rest:pat )* $(,)? ) => {
        (
            _,
            (
                $crate::destructure_quote!($first),
                ($crate::destructure_curried_args!( $( $rest ),* ), ()),
            ),
        )
    };
}
