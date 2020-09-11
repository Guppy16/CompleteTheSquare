function move_made(buttObj) {
    var move = buttObj.id;
    var curr_game_name = buttObj.name;
    var data = {"move": move, "curr_game_name": curr_game_name};
    as_json = JSON.stringify(data);
    console.log(as_json);

    // POST
    fetch('/passData', {

        // Specify the method
        method: 'POST',

        // A JSON payload
        body: as_json,
        cache: "no-cache",
        headers: new Headers({
          "content-type": "application/json"
        })
    }).then(function (response) { // At this point, Flask has printed our JSON
        return response.json();
    }).then(function (json) {

        console.log('POST response: ');

        // Should be 'OK' if everything was successful
        console.log(JSON.stringify(json));
    });

    // Get all player 1 positions
    // Get all player 2 positions

    // fetch('/passData')
    //     .then(function (response) {
    //         return response.json();
    //     })
    //     .then(function (json) {
    //         p1_pos = json["p1"];
    //         p2_pos = json["p2"];
    //         console.log(p1_pos);
    //     });


    // Make correct counters appear for each player
};