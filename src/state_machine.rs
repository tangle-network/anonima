pub enum State {
	// The node is idle when we don't have enough
	// participants for the DKG
	Idle,
	// The node enters the setup phase once enough
	// participants join and connect to the node.
	//
	// In the setup phase, the node will generate
	// its share and broacast the necessary data
	// to the network.
	Setup,
}
