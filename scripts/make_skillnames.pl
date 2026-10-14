#!/usr/bin/env perl

use v5.36;
use utf8;
use open qw(:std :encoding(UTF-8));

use DBI;
use DBD::SQLite::Constants qw(:file_open :dbd_sqlite_string_mode);
use JSON::PP;

my $json = JSON::PP->new;
$json->canonical(1);

@ARGV == 2 or die "Usage: make_skillnames.pl master.mdb skillnames.json\n";
my ( $mastermdb, $filename ) = @ARGV;

my $names = do {
    open my $fh, '<', $filename or die "Cannot open $filename: $!";
    local $/;
    my $data = $json->decode(<$fh>);
    close $fh;
    $data;
};

my $db = DBI->connect(
    "dbi:SQLite:$mastermdb",
    undef, undef,
    {
        RaiseError         => 1,
        sqlite_open_flags  => SQLITE_OPEN_READONLY,
        sqlite_string_mode => DBD_SQLITE_STRING_MODE_UNICODE_STRICT,
    }
);

my $select = $db->prepare(
    <<"SQL"
SELECT [index], text
  FROM text_data
  WHERE category = 47;
SQL
);

$select->execute;

my ( $id, $jpname );

$select->bind_columns( \( $id, $jpname ) );

my %ennames = map { $_->{id} => $_->{name_en} } $names->@*;

my %skills;

while ( $select->fetch ) {
    my $enname = $ennames{$id} // '';

    $skills{$id} = [ $jpname, $enname ];
    # Add inherited versions of uniques.
    if ( $id =~ /^1(\d+)/ ) {
        $skills{ '9' . $1 } = [ $jpname . '（継承）', $enname . ' (inherited)' ];
    }
}

say $json->encode( \%skills );
